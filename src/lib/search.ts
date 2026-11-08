import type { Album, Artist, Playlist, Track } from '@/bindings'

/** How many matches of each kind the search shows. */
const LIMITS = { songs: 5, artists: 4, albums: 4, playlists: 4 } as const

/** Lower case without accents, so "Bjork" finds "Björk". */
function normalize(text: string) {
  return text
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase()
}

/** Words of a text: runs of letters or digits, so "AC/DC" gives "ac" and "dc". */
function wordsOf(text: string) {
  return normalize(text).match(/[\p{L}\p{N}]+/gu) ?? []
}

/** A query split into words; an item matches when every word matches some word of its fields, in any order. */
type Terms = { whole: string; words: string[] }

function terms(query: string): Terms {
  return { whole: normalize(query.trim()), words: wordsOf(query) }
}

/** Typos a query word of this length may contain: none in short words, where any typo makes another word, then one, then two. */
function typosAllowed(word: string) {
  if (/^\d+$/.test(word) || word.length < 4) return 0
  return word.length < 7 ? 1 : 2
}

/** Fewest edits (insert, delete, replace, swap of neighbours) turning `word` into the start of `target`. */
function prefixDistance(word: string, target: string) {
  const rows = [Array.from({ length: target.length + 1 }, (_, j) => j)]
  for (let i = 1; i <= word.length; i++) {
    const row = [i]
    for (let j = 1; j <= target.length; j++) {
      const replace = (rows[i - 1]?.[j - 1] ?? 0) + (word[i - 1] === target[j - 1] ? 0 : 1)
      let cost = Math.min((rows[i - 1]?.[j] ?? 0) + 1, (row[j - 1] ?? 0) + 1, replace)
      if (i > 1 && j > 1 && word[i - 1] === target[j - 2] && word[i - 2] === target[j - 1])
        cost = Math.min(cost, (rows[i - 2]?.[j - 2] ?? 0) + 1)
      row.push(cost)
    }
    rows.push(row)
  }
  // the last row holds the cost against every prefix of the target, the best of which is the answer
  return Math.min(...(rows[word.length] ?? []))
}

/** Cost of matching one query word against one word of an item, lower is better: 0 when the item word starts with it, 1 when it sits inside it, 2 and up per typo; null when it does not match. */
function wordCost(query: string, target: string) {
  if (target.startsWith(query)) return 0
  if (target.includes(query)) return 1
  const allowed = typosAllowed(query)
  if (allowed === 0) return null
  // a typo in the first letter is rare and lets far too much through
  if (query[0] !== target[0] && query[1] !== target[1]) return null
  const typos = prefixDistance(query, target)
  return typos <= allowed ? 1 + typos : null
}

/** Rank of an item, lower is better: the costs of its best word matches, with a head start when its name starts with the whole query and a penalty for words found outside the name; null when some query word matches nothing. */
function rank({ whole, words }: Terms, name: string, others: (string | number | null)[]) {
  const nameWords = wordsOf(name)
  const otherWords = others.flatMap((field) => (field === null ? [] : wordsOf(String(field))))
  let total = 0
  for (const word of words) {
    const inName = Math.min(...nameWords.map((target) => wordCost(word, target) ?? Infinity))
    const inOthers = Math.min(...otherWords.map((target) => wordCost(word, target) ?? Infinity)) + 0.5
    const cost = Math.min(inName, inOthers)
    if (cost === Infinity) return null
    total += cost
  }
  return normalize(name).startsWith(whole) ? total - 1 : total
}

/** Items that match, best rank first and alphabetical within a rank, cut to `limit`. */
function best<T>(items: T[], limit: number, score: (item: T) => number | null, name: (item: T) => string) {
  return items
    .flatMap((item) => {
      const value = score(item)
      return value === null ? [] : [{ item, value }]
    })
    .sort((a, b) => a.value - b.value || name(a.item).localeCompare(name(b.item)))
    .slice(0, limit)
    .map(({ item }) => item)
}

/** Items whose name matches the query best first, for the suggestions of a field; an empty query matches nothing. */
export function suggest<T>(query: string, items: T[], limit: number, name: (item: T) => string) {
  const parsed = terms(query)
  if (parsed.words.length === 0) return []
  return best(items, limit, (item) => rank(parsed, name(item), []), name)
}

/** Test of whether a name matches the query, for filtering a list in place; an empty query lets everything through. */
export function matcher(query: string) {
  const parsed = terms(query)
  return (name: string) => parsed.words.length === 0 || rank(parsed, name, []) !== null
}

export type Library = { tracks: Track[]; albums: Album[]; artists: Artist[]; playlists: Playlist[] }

/** Best matches of each kind; songs also match their album's year, so "1987" finds the songs of that year's albums. */
export function searchLibrary(query: string, { tracks, albums, artists, playlists }: Library) {
  const parsed = terms(query)
  if (parsed.words.length === 0) return null
  const yearOf = new Map(albums.map((album) => [album.id, album.year]))
  return {
    songs: best(
      tracks,
      LIMITS.songs,
      (track) =>
        rank(parsed, track.title, [
          ...track.artists.map((artist) => artist.name),
          track.album,
          yearOf.get(track.albumId) ?? null,
        ]),
      (track) => track.title,
    ),
    artists: best(
      artists,
      LIMITS.artists,
      (artist) => rank(parsed, artist.name, []),
      (artist) => artist.name,
    ),
    albums: best(
      albums,
      LIMITS.albums,
      (album) => rank(parsed, album.title, [album.artist, album.year]),
      (album) => album.title,
    ),
    playlists: best(
      playlists,
      LIMITS.playlists,
      (playlist) => rank(parsed, playlist.name, []),
      (playlist) => playlist.name,
    ),
  }
}

export type SearchResults = NonNullable<ReturnType<typeof searchLibrary>>
