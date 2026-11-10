import type {
  AppError,
  DownloadError,
  LibraryError,
  Missing,
  PinError,
  PlaybackError,
  PlaylistError,
  TagError,
} from '@/bindings'
import { commands as bindings } from '@/bindings'

/** A backend command as it was called: its name in the bindings and the arguments it got. */
export type CommandCall = { readonly command: keyof typeof bindings; readonly args: readonly unknown[] }

/** The call each settled command result came from, so that a failure can tell what was asked. */
const calls = new WeakMap<object, CommandCall>()

const isCommand = (key: PropertyKey): key is keyof typeof bindings => typeof key === 'string' && key in bindings

/** The backend commands of the bindings, each noting the call its result or rejection came from. */
export const commands = new Proxy(bindings, {
  get(target, key, receiver) {
    const command: unknown = Reflect.get(target, key, receiver)
    if (typeof command !== 'function' || !isCommand(key)) return command
    return (...args: unknown[]) => {
      const result: unknown = command(...args)
      const note = (settled: unknown) => {
        if (typeof settled === 'object' && settled !== null) calls.set(settled, { command: key, args })
      }
      // noted before any caller's own handlers run, since those are attached to the same promise later
      if (result instanceof Promise) result.then(note, note)
      return result
    }
  },
})

/** A rejected backend command, carrying the typed reason next to a readable message, and the call when it is known. */
export class CommandError extends Error {
  override readonly name = 'CommandError'

  constructor(
    readonly error: AppError,
    readonly call: CommandCall | null = null,
  ) {
    super(errorMessage(error))
  }
}

/** Result shape tauri-specta wraps around every fallible command. */
type CommandResult = { status: 'ok'; data: unknown } | { status: 'error'; error: AppError }

/** Returns the data of a fallible command or throws its error as a `CommandError`. */
export async function unwrap<const R extends CommandResult>(result: Promise<R>) {
  const settled = await result
  if (settled.status === 'error') throw new CommandError(settled.error, calls.get(settled) ?? null)
  return settled.data as Extract<R, { status: 'ok' }>['data'] // narrowing a generic union by its tag does not reach the type parameter
}

function missingMessage(missing: Missing) {
  switch (missing.kind) {
    case 'track':
      return 'This song is no longer in the library'
    case 'trackFile':
      return `${missing.key} is not in the library`
    case 'album':
      return 'This album is no longer in the library'
    case 'artist':
      return 'This artist is no longer in the library'
    case 'playlist':
      return 'This playlist no longer exists'
    case 'folder':
      return 'This folder is no longer in the library'
    case 'folderPath':
      return `${missing.key} is not a library folder`
    case 'record':
      return 'A stored record is missing'
    default:
      return missing satisfies never
  }
}

function libraryMessage(reason: LibraryError) {
  switch (reason.type) {
    case 'notAbsolute':
      return 'The folder path must be absolute'
    case 'alreadyAdded':
      return 'This folder is already in the library'
    case 'overlaps':
      return `This folder overlaps ${reason.existing}, which is already in the library`
    case 'notInLibrary':
      return 'This folder is not in the library'
    default:
      return reason satisfies never
  }
}

function playlistMessage(reason: PlaylistError) {
  switch (reason.type) {
    case 'nameEmpty':
      return 'Playlist name cannot be empty'
    case 'nameTooLong':
      return `Playlist name can be at most ${reason.max} characters`
    case 'trackNotInPlaylist':
      return 'This song is no longer in the playlist'
    default:
      return reason satisfies never
  }
}

function pinMessage(reason: PinError) {
  switch (reason.type) {
    case 'notPinned':
      return 'This is not pinned'
    default:
      return reason.type satisfies never
  }
}

function playbackMessage(reason: PlaybackError) {
  switch (reason.type) {
    case 'volumeOutOfRange':
      return 'Volume must be between 0 and 100'
    case 'nothingToPlay':
      return 'There is nothing to play'
    case 'entryNotInQueue':
      return 'This song is no longer in the queue'
    default:
      return reason satisfies never
  }
}

function downloadMessage(reason: DownloadError) {
  switch (reason.type) {
    case 'unsupportedLink':
      return 'This is not a YouTube, YouTube Music or Spotify song link'
    case 'collectionLink':
      return 'Playlist, album and artist links are not supported yet, paste a link to one song'
    case 'noMatch':
      return 'No matching song was found on YouTube Music'
    case 'unsupportedPlatform':
      return 'Downloading is not available on this operating system'
    default:
      return reason satisfies never
  }
}

function tagMessage(reason: TagError) {
  switch (reason.type) {
    case 'titleEmpty':
      return 'The song needs a title'
    case 'artistEmpty':
      return 'The song needs an artist'
    case 'yearInvalid':
      return 'A year has four digits'
    case 'numberInvalid':
      return `Track and disc numbers go from 1 to ${reason.max}`
    default:
      return reason satisfies never
  }
}

/** Human-readable text of any backend error. */
export function errorMessage(error: AppError) {
  switch (error.type) {
    case 'library':
      return libraryMessage(error.reason)
    case 'playlist':
      return playlistMessage(error.reason)
    case 'playback':
      return playbackMessage(error.reason)
    case 'download':
      return downloadMessage(error.reason)
    case 'tags':
      return tagMessage(error.reason)
    case 'pin':
      return pinMessage(error.reason)
    case 'songFile':
      return `The song file cannot be changed: ${error.reason}`
    case 'fetch':
      return error.reason
    case 'notFound':
      return missingMessage(error.reason)
    case 'storage':
      return `Storage failed: ${error.reason}`
    case 'audio':
      return `Playback failed: ${error.reason}`
    case 'image':
      return `This image cannot be used: ${error.reason}`
    default:
      return error satisfies never
  }
}
