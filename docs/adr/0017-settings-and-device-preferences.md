# ADR 0017: Settings and device preferences

Status: Accepted for Slice 9b.

The application owns the settings store and shares it with account refreshes. Read generations prevent a response begun before a successful save from restoring an old default. Editor generations also protect a newer editor from an older save. Creation checks the fresh private overview and refuses duplicate names, including the permission hook. A failed usage request keeps the cached period's labels with its data.

Settings keeps the existing workbench palette, controls and modal interaction. Thirteen sections share one cached overview with the account store. A focused settings store owns defaults drafts, serialized saves and private stack editors. Refreshes retain dirty fields and editor text; editor reads have their own generation and loading state. Invalid names, numeric ranges and JSON objects are rejected before writes. HTTP and adapter validation remain authoritative for paths and permission rules. Imports and deletion affect only antiquota's stack.

The usage view renders bars with Chart.js directly, registering only the bar controller, bar element and its scales. It destroys the chart when the view unmounts or its data changes. HTML day targets expose empty days and keyboard focus; custom HTML tooltips stay inside the plot. Ledger totals remain exact; only the displayed mean is rounded. Period responses use generations so a late reading cannot replace a newer period.

Transcript and notification preferences use the existing localStorage keys and tolerate blocked or malformed storage. Grouped mode folds activity; every-step mode renders the rows independently. Detailed mode includes previews and reasoning; timestamp visibility changes without replacing history. Browser notification delivery checks permission, the selected event's preference and window focus, and cannot interrupt application work. Native zoom, autostart, notifications, external opening and directory reveal will use the Tauri adapter in Slice 10; browser mode applies no CSS scale.

The command reference is shared data for settings and the upcoming composer. Account cards, rotation and sign-in reuse their existing stores and tests. Account pill navigation now opens Usage when accounts exist and sign-in otherwise. One application event connection continues to serve these views.
