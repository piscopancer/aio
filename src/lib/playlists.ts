import { useMutation, useQuery } from '@tanstack/react-query'
import { mutations, queries } from '@/lib/query'

/** Numbered default name of the next playlist, which the user can change right away or later. */
export function useNextPlaylistName() {
  const { data: playlists = [] } = useQuery(queries.playlists())
  return `My Playlist #${playlists.length + 1}`
}

/** Creates a playlist with the default name and no image at once, for flows that cannot stop for a dialog. */
export function useCreatePlaylist() {
  const name = useNextPlaylistName()
  const { mutateAsync } = useMutation(mutations.createPlaylist())
  return async () => {
    const id = await mutateAsync({ name, ownCover: null })
    return { id, name }
  }
}
