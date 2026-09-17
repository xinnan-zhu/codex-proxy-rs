// 账号 turn_state 覆盖的三态控件与 wire 值之间的转换。
// wire 语义：null 不覆盖（透传）、'' 剥离发送、非空字符串强制为该值。
export type TurnStateOverrideMode = 'inherit' | 'strip' | 'force'

const MAX_TURN_STATE_OVERRIDE_LENGTH = 2048

export function turnStateOverrideModeOf(value: string | null): TurnStateOverrideMode {
  if (value === null)
    return 'inherit'
  return value === '' ? 'strip' : 'force'
}

export function turnStateOverrideFromMode(mode: string, value: string): string | null {
  if (mode === 'strip')
    return ''
  if (mode !== 'force')
    return null
  return value.trim()
}

export function turnStateOverrideInputError(mode: string, value: string): string | undefined {
  if (mode !== 'force')
    return undefined
  const trimmed = value.trim()
  if (!trimmed)
    return '请填写要强制发送的 Turn State 值，或改用“剥离发送”'
  if ([...trimmed].length > MAX_TURN_STATE_OVERRIDE_LENGTH || /\p{Cc}/u.test(trimmed))
    return `Turn State 覆盖值最多 ${MAX_TURN_STATE_OVERRIDE_LENGTH} 个字符，且不能包含控制字符`
}
