COOKIE HEAP LAB RESOURCE PACK

The application looks for this assets folder in the current working directory,
then beside the executable. If neither exists, it uses the copy embedded at
build time.

strings.txt
-----------
Edit the text after each '='. Do not rename the key on the left. Keep named
placeholders such as {count}, {price}, and {path}; the application replaces
them at runtime. Use \n for a line break.

Choose Appearance > Reload resources to apply text and sprite edits without
restarting the application.

*.sprite.txt
------------
Sprites use a plain text indexed-palette format. The size line is WIDTHxHEIGHT.
Each palette line assigns one character to an RRGGBBAA color. A palette color
with alpha 00 is transparent. The rows after "pixels:" must exactly match the
declared width and height.
