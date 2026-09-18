//! OpenAI 客户端协议的 Bearer API key 认证。

use axum::{
    http::{HeaderMap, header::AUTHORIZATION},
    response::{IntoResponse, Response},
};
use gateway_core::engine::execution::AuthenticatedClient;
use gateway_core::policy::{ClientVersionRejection, CodexClientKind, CodexClientVersion};

use super::{
    error::{
        client_version_rejection_response, missing_client_api_key_response,
        runtime_unavailable_response,
    },
    service::OpenAiService,
};

/// Client API key 鉴权失败原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientApiKeyAuthError {
    /// 缺失 Authorization 头。
    MissingAuthorization,
    /// Authorization 头不是合法的 Bearer token。
    MalformedAuthorization,
    /// Bearer token 不是 client API key 格式。
    InvalidKeyFormat,
    /// Key 不存在、已禁用或 wire 格式无效。
    InvalidKey,
    /// RuntimeSnapshot 一致性保护暂停接收新请求。
    RuntimeUnavailable,
}

/// Client API Key 或最低版本准入失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientAccessError {
    Authentication(ClientApiKeyAuthError),
    Version(ClientVersionRejection),
}

impl From<ClientApiKeyAuthError> for ClientAccessError {
    fn from(error: ClientApiKeyAuthError) -> Self {
        Self::Authentication(error)
    }
}

impl From<ClientVersionRejection> for ClientAccessError {
    fn from(error: ClientVersionRejection) -> Self {
        Self::Version(error)
    }
}

/// 从有界请求头中识别出的客户端及其可选合法版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifiedCodexClient {
    kind: CodexClientKind,
    version: Option<CodexClientVersion>,
}

impl IdentifiedCodexClient {
    #[must_use]
    pub const fn kind(&self) -> CodexClientKind {
        self.kind
    }

    #[must_use]
    pub const fn version(&self) -> Option<&CodexClientVersion> {
        self.version.as_ref()
    }
}

impl ClientApiKeyAuthError {
    /// 返回可用于日志和指标的稳定失败原因。
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::MissingAuthorization => "missing_authorization",
            Self::MalformedAuthorization => "malformed_authorization",
            Self::InvalidKeyFormat => "invalid_key_format",
            Self::InvalidKey => "invalid_key",
            Self::RuntimeUnavailable => "runtime_unavailable",
        }
    }
}

/// 从请求头提取 Bearer Client API key。
///
/// # Errors
///
/// Header 缺失、Bearer 语法错误或 Key 不能作为 HTTP Bearer 值时返回稳定错误。
pub fn bearer_client_api_key(headers: &HeaderMap) -> Result<&str, ClientApiKeyAuthError> {
    let raw = headers
        .get(AUTHORIZATION)
        .ok_or(ClientApiKeyAuthError::MissingAuthorization)?
        .to_str()
        .map_err(|_| ClientApiKeyAuthError::MalformedAuthorization)?;
    let token = raw
        .strip_prefix("Bearer ")
        .ok_or(ClientApiKeyAuthError::MalformedAuthorization)?
        .trim();
    if token.is_empty() {
        return Err(ClientApiKeyAuthError::MalformedAuthorization);
    }
    if gateway_core::policy::PlaintextClientApiKey::validate(token).is_err() {
        return Err(ClientApiKeyAuthError::InvalidKeyFormat);
    }
    Ok(token)
}

pub(crate) fn authenticate_client(
    service: &OpenAiService,
    headers: &HeaderMap,
) -> Result<AuthenticatedClient, ClientAccessError> {
    let key = bearer_client_api_key(headers)?;
    let client = service.authenticate(key)?;
    if let Some(identified) = identify_codex_client(headers) {
        client
            .snapshot()
            .min_codex_client_versions()
            .enforce(identified.kind(), identified.version())?;
    }
    Ok(client)
}

pub(crate) fn client_access_error_response(error: ClientAccessError) -> Response {
    match error {
        ClientAccessError::Authentication(error) => {
            log_client_api_key_auth_failure(error);
            if error == ClientApiKeyAuthError::RuntimeUnavailable {
                runtime_unavailable_response().into_response()
            } else {
                missing_client_api_key_response().into_response()
            }
        }
        ClientAccessError::Version(error) => {
            tracing::info!(
                client = error.kind().as_str(),
                current_version = error.current().map(ToString::to_string),
                min_version = %error.min(),
                "Codex client version requirement rejected request"
            );
            client_version_rejection_response(&error).into_response()
        }
    }
}

/// Desktop 优先于内嵌 CLI/Core；未知客户端及未提供应用版本的手机远程客户端返回 `None`。
#[must_use]
pub fn identify_codex_client(headers: &HeaderMap) -> Option<IdentifiedCodexClient> {
    const MAXIMUM_HEADER_LENGTH: usize = 4096;

    let originator = bounded_ascii_header(headers, "originator", MAXIMUM_HEADER_LENGTH);
    let user_agent = bounded_ascii_header(headers, "user-agent", MAXIMUM_HEADER_LENGTH);
    let is_desktop = originator.is_some_and(|value| value.eq_ignore_ascii_case("Codex Desktop"))
        || user_agent.is_some_and(|value| contains_ascii_case_insensitive(value, "Codex Desktop"));
    if is_desktop {
        let explicit_version = bounded_ascii_header(headers, "version", MAXIMUM_HEADER_LENGTH);
        let user_agent_version = user_agent.and_then(desktop_version_from_user_agent);
        // 手机远程初始化会覆盖进程级 UA 后缀，保留的 Desktop/Core 前缀不代表应用版本。
        // 仅缺失版本信息时免于门禁；已提供的版本即使非法，也继续按 Desktop 校验。
        if !headers.contains_key("version")
            && user_agent_version.is_none()
            && headers
                .get("user-agent")
                .is_some_and(|value| value.as_bytes().len() <= MAXIMUM_HEADER_LENGTH)
            && user_agent.is_some_and(has_chatgpt_remote_user_agent_suffix)
        {
            return None;
        }
        let version = match explicit_version {
            Some(value) => CodexClientVersion::parse(value).ok(),
            None => user_agent_version.and_then(|value| CodexClientVersion::parse(value).ok()),
        };
        return Some(IdentifiedCodexClient {
            kind: CodexClientKind::Desktop,
            version,
        });
    }

    let user_agent = user_agent?;
    for product in user_agent.split(|character: char| {
        character.is_ascii_whitespace() || matches!(character, '(' | ')' | ';' | ',' | '[' | ']')
    }) {
        for prefix in ["codex_cli_rs", "codex-cli"] {
            if product.eq_ignore_ascii_case(prefix) {
                return Some(IdentifiedCodexClient {
                    kind: CodexClientKind::Cli,
                    version: None,
                });
            }
            let Some((name, version)) = product.split_once('/') else {
                continue;
            };
            if name.eq_ignore_ascii_case(prefix) {
                return Some(IdentifiedCodexClient {
                    kind: CodexClientKind::Cli,
                    version: CodexClientVersion::parse(version).ok(),
                });
            }
        }
    }
    None
}

/// Codex 官方客户端家族 UA 前缀（镜像 sub2api `codexOfficialClientUAPrefixes`，
/// 取证自 codex-rs `is_first_party_originator`）。仅用于 codex_only 账号门控的
/// 身份判定，与最低版本策略的 `identify_codex_client` 解耦。
const OFFICIAL_CODEX_UA_PREFIXES: &[&str] = &[
    "codex_cli_rs/",
    "codex-tui/",
    "codex_vscode/",
    "codex_vscode_copilot/",
    "codex_app/",
    "codex_chatgpt_desktop/",
    "codex_atlas/",
    "codex_exec/",
    "codex_sdk_ts/",
];

/// `Codex ` 前缀家族（如 `Codex Desktop/…`）。保留尾随空格，避免归一化后
/// 退化为裸 `codex` 而把任意包含 codex 的 UA 放行。
const OFFICIAL_CODEX_FAMILY_PREFIX: &str = "codex ";

/// 官方 originator 精确集（镜像 sub2api `codexOfficialClientOriginators`）；
/// app-server `initialize` 会把 clientInfo.name 逐字写入 originator。
const OFFICIAL_CODEX_ORIGINATORS: &[&str] = &[
    "codex_cli_rs",
    "codex-tui",
    "codex_vscode",
    "codex_vscode_copilot",
    "codex_app",
    "codex_chatgpt_desktop",
    "codex_atlas",
    "codex_exec",
    "codex_sdk_ts",
];

/// 识别「官方 Codex 客户端」身份，供 codex_only 账号门控使用。
///
/// 命中 `identify_codex_client` 的客户端直接沿用其结果（Desktop/CLI）；其余
/// 官方家族（codex-tui、codex_vscode、codex_exec、codex_atlas 等）归为 `Cli`。
/// 与最低版本策略无关：未识别的官方家族客户端不受 min version 门禁约束。
#[must_use]
pub fn official_codex_client(headers: &HeaderMap) -> Option<CodexClientKind> {
    const MAXIMUM_HEADER_LENGTH: usize = 4096;

    if let Some(identified) = identify_codex_client(headers) {
        return Some(identified.kind());
    }
    let originator = bounded_ascii_header(headers, "originator", MAXIMUM_HEADER_LENGTH);
    let user_agent = bounded_ascii_header(headers, "user-agent", MAXIMUM_HEADER_LENGTH);
    if originator.is_some_and(official_codex_originator)
        || user_agent.is_some_and(official_codex_ua)
    {
        return Some(CodexClientKind::Cli);
    }
    None
}

/// originator 判定：精确集 + `Codex ` 家族前缀，不用「含 codex」宽松兜底。
fn official_codex_originator(originator: &str) -> bool {
    let value = originator.trim().to_ascii_lowercase();
    if value.is_empty() {
        return false;
    }
    OFFICIAL_CODEX_ORIGINATORS.contains(&value.as_str())
        || value.starts_with(OFFICIAL_CODEX_FAMILY_PREFIX)
}

/// UA 判定：官方前缀集（仅前缀匹配，收窄伪造面）+ `Codex ` 家族前缀 +
/// 尾部括号组兜底——codex-rs 会把 clientInfo.name 写入 UA 末尾 `(name; version)`，
/// 前缀被 originator override 改写时仍可恢复真实客户端身份。
fn official_codex_ua(user_agent: &str) -> bool {
    let ua = user_agent.trim().to_ascii_lowercase();
    if ua.is_empty() {
        return false;
    }
    if OFFICIAL_CODEX_UA_PREFIXES
        .iter()
        .any(|prefix| ua.starts_with(prefix))
    {
        return true;
    }
    if ua.starts_with(OFFICIAL_CODEX_FAMILY_PREFIX) {
        return true;
    }
    official_ua_trailer_name(&ua).is_some_and(|name| official_codex_originator(&name))
}

/// 提取 codex-rs 格式 UA 最后一个括号组里的 clientInfo.name（`;` 之前部分）。
fn official_ua_trailer_name(ua: &str) -> Option<String> {
    let last = ua.rfind('(')?;
    let rest = &ua[last + 1..];
    let close = rest.find(')')?;
    let name = rest[..close].trim().split(';').next().unwrap_or("").trim();
    if name.is_empty() {
        return None;
    }
    Some(name.to_ascii_lowercase())
}

fn bounded_ascii_header<'a>(
    headers: &'a HeaderMap,
    name: &'static str,
    maximum_length: usize,
) -> Option<&'a str> {
    let value = headers.get(name)?.to_str().ok()?;
    if !value.is_ascii() {
        return None;
    }
    Some(&value[..value.len().min(maximum_length)])
}

fn desktop_version_from_user_agent(user_agent: &str) -> Option<&str> {
    let marker = "Codex Desktop;";
    let start = find_ascii_case_insensitive(user_agent, marker)? + marker.len();
    // 空候选也代表已提供应用版本，不能被手机远程的缺失版本规则放行。
    user_agent[start..]
        .trim_start()
        .split([')', ' ', ';', ','])
        .next()
}

fn has_chatgpt_remote_user_agent_suffix(user_agent: &str) -> bool {
    let Some((_, suffix)) = user_agent
        .strip_suffix(')')
        .and_then(|value| value.rsplit_once(" ("))
    else {
        return false;
    };
    let Some((name, version)) = suffix.split_once("; ") else {
        return false;
    };
    let Some(platform) = name
        .strip_prefix("codex_chatgpt_")
        .and_then(|value| value.strip_suffix("_remote"))
    else {
        return false;
    };
    [platform, version].into_iter().all(|value| {
        !value.is_empty()
            && !value.contains(['(', ')', ';'])
            && !value.bytes().any(|byte| byte.is_ascii_whitespace())
    })
}

fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    find_ascii_case_insensitive(haystack, needle).is_some()
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn log_client_api_key_auth_failure(error: ClientApiKeyAuthError) {
    match error {
        ClientApiKeyAuthError::RuntimeUnavailable => {
            tracing::warn!(
                auth_failure = error.reason(),
                "Client API key authorization failed"
            );
        }
        _ => {
            tracing::info!(
                auth_failure = error.reason(),
                "Client API key authorization failed"
            );
        }
    }
}
