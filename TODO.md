# TODO

## A new app icon

The current icon in `app-icon.svg` is too plain and gets lost among other icons in the taskbar and the Start menu. The app needs a more memorable icon that stays in the peach palette of the interface, so that the icon and the accent colour read as one brand. Once the new SVG is ready, the platform icons in `src-tauri/icons` have to be regenerated from it with `tauri icon`.

## Cover search in the tag editor

In the song tag editor dialog the user should be able to find a cover on the internet without leaving the app and without downloading a picture by hand first. A "Find cover" button next to the picture, or a "Show variants" item in the right-click menu of the picture, opens a panel with many candidate covers at once, found by the artist and album (or the title, when there is no album) of the song. The user clicks one of the variants and it becomes the cover in the form, exactly as if a local file had been picked, so it is written to the file only when the dialog is saved.

It still needs to be settled which sources the search uses (for example the iTunes Search API, MusicBrainz with the Cover Art Archive, Deezer), how many variants to show, and whether the user can correct the search query before searching.
