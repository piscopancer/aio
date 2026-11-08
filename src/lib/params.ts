import { notFound } from '@tanstack/react-router'

/** Route param codec for a numeric entity id; a malformed id is a not-found page. */
export function idParam<const K extends string>(key: K) {
  return {
    parse: (params: Record<K, string>) => {
      const id = Number(params[key])
      if (!Number.isSafeInteger(id) || id <= 0) throw notFound()
      return { [key]: id } as Record<K, number> // a computed key widens to an index signature
    },
    stringify: (params: Record<K, number>) => ({ [key]: String(params[key]) }) as Record<K, string>, // same widening as above
  }
}
