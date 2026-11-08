import { useEffect, useState } from 'react'

/** `value` once it has stayed the same for `delayMs`, so typing does not start work on every key press. */
export function useDebounced<const T>(value: T, delayMs: number) {
  const [settled, setSettled] = useState(value)
  useEffect(() => {
    const timer = setTimeout(() => setSettled(value), delayMs)
    return () => clearTimeout(timer)
  }, [value, delayMs])
  return settled
}
