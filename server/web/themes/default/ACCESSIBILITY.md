# Default theme access checks

The Zest theme uses a primary navigation landmark, a main content region, and a
persistent playback region. A skip link moves to the main region. Each page has
a heading. Loading and connection messages use status roles. Request errors use
an alert role and include a Retry control.

## Keyboard controls

Links, native inputs, and Zest controls support keyboard use. The active route
has `aria-current="page"`. Sliders have labels. Ctrl+K or Command+K opens search.
Escape closes the queue or mobile navigation drawer. The drawer uses a native
modal dialog with contained keyboard focus. Close and Escape return focus to
Menu. A navigation link closes the drawer and focuses the main content. Zest
dialogs manage focus while open.

Track rows use the Zest ListView. Queue actions have labels that include the
track name. Move up, Move down, Remove, and Undo do not need a drag operation.
Nested action keys stop at the action group. This works around a Zest ListView
issue that otherwise takes Space from a button. The upstream request is in
`../zest/component-bugs.md`, relative to the Ichoi repository.

The player and device selector remain visible during navigation and queue edits.
A More dialog exposes controls that do not fit on narrow screens. Playback
position updates do not replace unchanged queue rows.

## Appearance

The theme supports system, light, and dark modes. Focus has a visible outline.
Status text accompanies state colors. Reduced-motion settings disable animation
and transitions. Cover placeholders show the item title when no image is present.
The responsive layout keeps the player below the main scroll area.

## Current limits

The interface is English. A complete screen-reader audit and an automated
contrast audit have not been completed for the new theme. Browser checks cover
320-pixel and 390-pixel mobile layouts, catalog errors, retry, search retention,
and queue edits.
These checks are not a WCAG certification.

## Next Actions

Check keyboard navigation, dialogs, and virtual lists with a screen reader on
Firefox before release. Check long titles and large text on physical phones.
