# Say

A picture talker for ages 2 and up. This is the native app. The browser page is not kept. Rust keeps it memory-safe, and the release build is a native window.

The child uses the board. A parent sets the stage and the skill. The board does not change stage by itself.

The skill is how many words are on the board. Starting is the first set. Everyday, Wider, and Full add words. A word file can say `"level": 2` so it waits for Everyday. A missing level stays in Starting. Words above the skill are left off. They are not empty spots. The choice is kept with the other parent settings and does not reset the turn count.

Pictures and the words they speak live in `content`, one json file and one picture per word. A photograph named like the word, such as `mom.jpg`, is used instead of the symbol. PNG and SVG need no extra tool. JPEG, GIF, WebP, AVIF, BMP, and TIFF use ImageMagick. Most symbols are Mulberry Symbols, Creative Commons Attribution-ShareAlike 4.0. Credit and the file list are in `content/SOURCES.txt`. Categories, the order of the board, and what follows a finished line are files in that same folder. Open Say again after you add a word.

## Stages

A picture adds its word. On First messages, and for a thing at the start of Growing phrases, one picture can still open a short request such as "I want water". After that, each picture adds one word. The line stays until Clear. Nothing starts a new sentence by itself.

Hear it plays the whole line in the headphones. Each new word plays there too, and only the new word. Out loud plays the whole line on the other speaker. Setup lists every connected speaker twice: Hear for the headphones, and Out loud for the room. The chosen one is highlighted, and Say speaks "Headphones" or "Speakers" on that device so you can hear which is which. A headset is chosen for Hear, and the built-in speakers for Out loud, until you pick different ones.

The Next row is a fresh guess from the word graph each time the line changes. It uses the links in the word files, lines you have said out loud, the job of the last word, and other words from the same group. It stays on this computer. It is not a language model. On Predicted words, More words lengthens that list. Other words opens the categories for a word the guess missed. Pause, on the bottom row, puts `<-->` in the line. Hear it and Out loud rest there instead of saying the word pause. Setup can make speech slower or faster. Usual is the speed the board already uses. Word graph is the guesser the board ships with. Llama is a test that asks a small model on this computer, through llama.cpp, for the next pictures. The word graph stays up until that model answers. Set `SAY_LLAMA_MODEL` to choose a different `.gguf` file.

Predicted words: the board fills with pictures that fit the line so far. It uses the same guess as the Next row. The graph of lines said out loud is `predict.json` beside the settings file. Reset is deleting that file. Other words opens the categories when the guess is wrong. Nothing about the line is sent off the machine.

Back up removes the last piece and leaves the rest. Clear empties the line and leaves the earlier turns. A tap on Setup opens setup. A look still has to hold there.

Simple is the calm board. Future is the space-age board: raised buttons the starfield shows through, a dark field, and a glow while a look is about to land. Setup lists voices. Adam is the ordinary American voice. The others are Michael, Jack, Britain, Scotland, New York, Caribbean, Alicia, Annie, Linda, Auntie, and Grandma. Choosing one speaks that name in the headphones. The choice is saved with the other parent settings.

## Modes

User mode is what opens. Yes and No stay large.

Setup is for a parent. Tap **Setup**, or press Ctrl+P. A short look does not open it; the look has to hold, and the button fills while it does. Pick a stage, a skill, Simple or Future, and the two speakers, then turn words on or off, then **Done. Open the board**. Above Starting, setup opens a group first, because the list is long. The setup screen counts finished messages and says when the next stage is ready. Mute is on both screens. Choices are saved in `$XDG_CONFIG_HOME/say/board.json`, or `~/.config/say/board.json`. Older files that say big, medium, or sentences still open, and they stay on Starting until you choose another skill.

Ctrl+Q quits. Esc leaves setup. On the child's home board, Esc leaves fullscreen.

## Gaze

The app talks to the tobiifreed daemon. It does not link that program.

It connects to `ws://127.0.0.1:7081` first and applies the screen warp the daemon sends. If that port is closed it uses `$XDG_RUNTIME_DIR/tobiifreed/gaze.sock` and reads `~/.config/tobii/screen_warp.json`. Mouse and touch still work when the daemon is down.

Start the daemon with the websocket, then open Say on the calibrated panel:

```
/home/joseph/code/tobii-eye/daemon/target/release/tobiifreed --ws 127.0.0.1:7081
```

Look until the green ring fills. Menu, categories, and sentence tools wait longer than a picture. The Parent control waits longer than those. A finger press wins over a look.

## Build

GTK 4 and espeak-ng are required.

```
cargo run --release
cargo run --release -- --setup
cargo run --release -- --selfcheck
```

Dwell times are 700 ms on a big picture, 900 on more words, 1100 on a sentence cell, 800 on Yes and No, 1300 on menus, and 2600 on Parent.
