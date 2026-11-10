# TODO

## A new app icon

The current icon in `app-icon.svg` is too plain and gets lost among other icons in the taskbar and the Start menu. The app needs a more memorable icon that stays in the peach palette of the interface, so that the icon and the accent colour read as one brand. Once the new SVG is ready, the platform icons in `src-tauri/icons` have to be regenerated from it with `tauri icon`.

## Finding songs to download by name

Getting a new song into the library is still done by hand: open some MP3 site, wait for its slow search, play a few seconds of each result to find the right recording, download the file and then move it into the library folder. A single song easily takes five to ten minutes this way. Since the app is meant for personal use only, it should take over this routine and search those sources by itself, the way a person would.

The download dialog behind the Download button would grow into a larger search view. A typed artist and title are sent to several sources at once, such as the popular Russian MP3 sites, VK Music and any catalogue that indexes MP3 files from many sites, and the results arrive in one list as each source answers. Every result shows the source it was found on, its length and bitrate where known, and a button that streams a short preview right in the app, so the right version can be picked without leaving it. One more click downloads the file straight into the library.

Before the work starts, the sources have to be looked over one by one: which of them give a usable search page or API, which need scraping, how VK Music can be reached, and how quickly each one answers. A first version with two or three dependable sources would already save most of the time, and further sources can be added one at a time later.

## A README for the project

The repository has no README yet, so anyone who opens it, including me after a few months away, has to piece together from the code what the app is and how to work on it. The README should open with a short description of Aio and a screenshot or two, then explain how to set up the development environment (Node with pnpm, Rust and the Tauri prerequisites for Windows), how to start the app with `pnpm dev` and how to build a release. A short section on the layout of the code, the Rust backend in `src-tauri` with its layers and the React frontend in `src`, together with the commands for regenerating the bindings and applying database migrations, would make the first hour with the project much easier. Writing it should take about a day.

## Named marks inside a song

Long tracks such as hour-long DJ mixes or albums uploaded as one file are hard to move around in: the only way to reach a favourite part is to drag the progress bar and listen until the right moment comes. I want to be able to set my own marks at chosen moments of a song, each with a name, for example "drop", "second track" or "chorus".

The marks would show on the progress bar of the player as small ticks, with the name in a tooltip on hover, and clicking one would jump there. Keyboard shortcuts would jump to the previous and the next mark from the current position, so a mix can be skipped through part by part without touching the mouse. Adding a mark should be possible while the song plays, at the current moment, with the name typed right away, and marks should be renameable, movable and removable afterwards. The marks belong to the song, so they would be kept in the database by track and survive restarts and rescans. A first version with adding, showing and jumping would take two or three days.
