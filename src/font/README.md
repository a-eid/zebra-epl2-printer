Place an Arabic-capable TrueType font here (not checked into repo due to licensing).

Recommended: Amiri-Regular.ttf

After adding, in the Tauri app you can include it with:
`let font = include_bytes!("../zebra-epl2-printer/src/font/Amiri-Regular.ttf");`
