import { useMutation, useQuery } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import {
  Disc3Icon,
  ListEndIcon,
  ListPlusIcon,
  MicVocalIcon,
  PencilIcon,
  PlayIcon,
  PlusIcon,
  Trash2Icon,
} from 'lucide-react'
import type { ReactNode } from 'react'
import type { PlaybackContext, Track } from '@/bindings'
import { Cover } from '@/components/cover'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { player } from '@/lib/player'
import { useCreatePlaylist, useOrderedPlaylists } from '@/lib/playlists'
import { editTrack } from '@/lib/track-editor'
import { mutations, queries } from '@/lib/query'

type TrackMenuProps = { track: Track; context: PlaybackContext }

/** Right-click menu of a track row. */
export function TrackMenu({ children, ...props }: TrackMenuProps & { children: ReactNode }) {
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        <TrackMenuItems {...props} />
      </ContextMenuContent>
    </ContextMenu>
  )
}

/** Mounted only while the menu is open, so closed rows run no queries. */
function TrackMenuItems({ track, context }: TrackMenuProps) {
  const { albumId } = track
  const navigate = useNavigate()
  // one artist is a plain item, several get a submenu to pick from
  const soleArtist = track.artists.length === 1 ? track.artists[0] : undefined
  const playlists = useOrderedPlaylists()
  const { data: holding = [] } = useQuery(queries.playlistsWithTrack(track.id))
  const addToPlaylist = useMutation(mutations.addToPlaylist())
  const removeFromPlaylist = useMutation(mutations.removeFromPlaylist())
  const createPlaylist = useCreatePlaylist()
  const trackIds = [track.id]

  if (track.missing)
    return (
      context.kind === 'playlist' && (
        <ContextMenuItem onSelect={() => removeFromPlaylist.mutate({ id: context.id, trackIds })}>
          <Trash2Icon />
          Remove from this playlist
        </ContextMenuItem>
      )
    )
  return (
    <>
      <ContextMenuItem onSelect={() => player.play(context, track.id)}>
        <PlayIcon />
        Play
      </ContextMenuItem>
      <ContextMenuItem onSelect={() => player.enqueue(trackIds)}>
        <ListEndIcon />
        Add to queue
      </ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuSub>
        <ContextMenuSubTrigger>
          <ListPlusIcon />
          Add to playlist
        </ContextMenuSubTrigger>
        <ContextMenuSubContent className="max-h-80">
          <ContextMenuItem
            onSelect={async () => {
              const { id, name } = await createPlaylist()
              addToPlaylist.mutate({ id, name, trackIds })
            }}
          >
            {/* a blank playlist cover with a plus on its corner, lined up with the covers of the playlists below */}
            <span className="relative -my-1">
              <Cover src={null} kind="playlist" className="size-7 rounded-sm" />
              <PlusIcon className="absolute -right-0.5 -bottom-0.5 size-3 rounded-full bg-muted-foreground p-px text-background" />
            </span>
            New playlist
          </ContextMenuItem>
          {playlists.length > 0 && <ContextMenuSeparator />}
          {playlists.map(({ id, name, cover }) => (
            // a playlist that already holds the song is shown greyed out, as there is nothing to add
            <ContextMenuItem
              key={id}
              disabled={holding.includes(id)}
              onSelect={() => addToPlaylist.mutate({ id, name, trackIds })}
            >
              <Cover src={cover} kind="playlist" className="-my-1 size-7 rounded-sm" />
              <span className="max-w-64 truncate">{name}</span>
            </ContextMenuItem>
          ))}
        </ContextMenuSubContent>
      </ContextMenuSub>
      {context.kind === 'playlist' && (
        <ContextMenuItem onSelect={() => removeFromPlaylist.mutate({ id: context.id, trackIds })}>
          <Trash2Icon />
          Remove from this playlist
        </ContextMenuItem>
      )}
      <ContextMenuSeparator />
      {soleArtist !== undefined ? (
        <ContextMenuItem onSelect={() => navigate({ to: '/artists/$artistId', params: { artistId: soleArtist.id } })}>
          <MicVocalIcon />
          Go to artist
        </ContextMenuItem>
      ) : (
        // a song credited to nobody has no artist to go to
        track.artists.length > 1 && (
          <ContextMenuSub>
            <ContextMenuSubTrigger>
              <MicVocalIcon />
              Go to artist
            </ContextMenuSubTrigger>
            <ContextMenuSubContent>
              {track.artists.map((artist) => (
                <ContextMenuItem
                  key={artist.id}
                  onSelect={() => navigate({ to: '/artists/$artistId', params: { artistId: artist.id } })}
                >
                  {artist.name}
                </ContextMenuItem>
              ))}
            </ContextMenuSubContent>
          </ContextMenuSub>
        )
      )}
      {albumId !== null && (
        <ContextMenuItem onSelect={() => navigate({ to: '/albums/$albumId', params: { albumId } })}>
          <Disc3Icon />
          Go to album
        </ContextMenuItem>
      )}
      <ContextMenuSeparator />
      <ContextMenuItem onSelect={() => editTrack(track)}>
        <PencilIcon />
        Edit details
      </ContextMenuItem>
    </>
  )
}
