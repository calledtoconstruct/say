# Say board content

The pictures and the words they speak live in this folder. The program does not keep its own copy.

## Add a word

1. Create `words/your-word.json`.
2. Put a picture beside it. The file name before the dot is the word's id. SVG and PNG need no extra tool. A photograph can be `.jpg`, `.jpeg`, `.png`, `.gif`, `.webp`, `.avif`, `.bmp`, `.tif`, or `.tiff`. JPEG, GIF, WebP, AVIF, BMP, and TIFF need ImageMagick. If a photograph and a symbol share the name, the photograph is the one that shows. Delete the photograph to use the symbol again.
3. Name the categories it belongs to inside the json, for example `"categories": ["food"]`.
4. To choose where it sits, add that id to the matching file in `categories/`. If you leave it out of the list, it is added at the end of each category it names.

Then open Say again. It reads this folder directly.

## Word file

```json
{
  "say": "water",
  "role": "object",
  "categories": ["needs", "food"],
  "phrase": ["i", "want", "water"],
  "next": { "please": 5 }
}
```

`say` is the spoken text. `role` is person, verb, object, feeling, place, polite, or social. `level` is 1, 2, 3, or 4. Leave it out and the word stays in the first set. A parent sets the skill in setup: Starting, Everyday, Wider, or Full. Words above that skill are left off the board. They are not empty spots. `phrase` is the whole line First messages speaks; leave it out to speak only this word. `next` is which words often follow, and how strongly. `starter` is how strongly it opens a line. `closes` true means this word can finish a thought on its own. `also` adds a second role, such as `["social"]`.

Changing the skill does not clear the turn count. Changing the stage does.

## Categories

`categories/food.txt` starts with the label, then `home` or `bar`, then the word ids in the order they appear. `home` is a picture on the main board. `bar` is the row at the bottom, used for yes and no. A line `tone object` sets the card color: person, verb, object, feeling, place, polite, or social.

`order.txt` lists the category ids in the order they appear. A category left out of that list is added at the end.

`follow.txt` is what the board offers after a finished line, one id per line. A line ending in `+` is offered even when that word is already in the sentence.

## Pictures

A clear shape on a transparent background works best. The label under the picture is the `say` text, not letters inside the image. Use a photograph of a person, a place, or a thing when you want that instead of the symbol. Name it with the word id, for example `words/mom.jpg`.

Most symbols are Mulberry Symbols by Steve Lee, Creative Commons Attribution-ShareAlike 4.0. See `SOURCES.txt` and `licenses/Mulberry-Symbols-LICENSE.txt`. Pictures drawn where Mulberry had no clear symbol use that same license. Share changes to those pictures with the same credit.

Predicted words learns from finished lines. That graph is not stored in this folder. On the native app it is `predict.json` next to the settings file. In the browser it stays in local storage.

The native app speaks any `say` text with espeak-ng. The web page uses the computer's voice when the browser has one. Its built-in clips cover the words that shipped with the board. A new phrase is spoken by the computer's voice, and any word the clips do not know is skipped when the browser has no voice.
