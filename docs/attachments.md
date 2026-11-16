# Attachments

A photo of a board taken at work, a screenshot of an error, a log, a PDF,
a short video: the panel attaches files to a prompt, from the phone or
the desk, and Claude opens them on the machine its session runs on. The
microphone dictates a prompt instead of typing it.

1. [Attaching](#1-attaching)
2. [What Claude gets](#2-what-claude-gets)
3. [Photos are made smaller](#3-photos-are-made-smaller)
4. [Files: what is kept, and for how long](#4-files-what-is-kept-and-for-how-long)
5. [How it works](#5-how-it-works)
6. [Turning it off, and the bounds](#6-turning-it-off-and-the-bounds)
7. [Dictation](#7-dictation)
8. [Recordings and videos](#8-recordings-and-videos)
9. [Not yet](#9-not-yet)

## 1. Attaching

In a session's message field, and in the sheet that starts a session:

- **+ › Photo or file** picks one or several (ten at most per prompt);
- **+ › Take a photo** opens the camera, on a phone or a tablet;
- **paste** a copied screenshot or file into the field (`Ctrl V`);
- **drop** files on the field.

Each file shows above the buttons with its size while it uploads; the ✕
cancels an upload or takes the file out again (and off the hub). Send
waits for the uploads. A prompt may be nothing but its files. Uploaded
files of a prompt you have not sent yet survive a reload on that device.

In the chat a prompt's pictures show as thumbnails that open large,
recordings and videos get a player, anything else a chip to download.

## 2. What Claude gets

Claude Code takes text from the panel and nothing else, so the prompt
names each file where Claude's own tools find it:

```text
What is on this board?

[Attached image: /home/you/.local/state/ccdash/attachments/3e2b…/board.jpg]
```

| Kind                           | What Claude does with it                                                                     |
| ------------------------------ | -------------------------------------------------------------------------------------------- |
| picture (JPEG, PNG, GIF, WebP) | sees it, by reading the file                                                                 |
| PDF, text, code                | reads it                                                                                     |
| sound, video                   | cannot hear or watch: it gets the words and still frames ([below](#8-recordings-and-videos)) |
| anything else                  | opens it with whatever tool fits, if one does                                                |

The kind is read from the file's first bytes, never from its name: an SVG
or a page is text to ccdash, so it is never drawn in the panel.

When a file cannot be put on the session's machine (the hub cannot be
reached from there, the file's time on the hub ran out), the prompt still
goes out, with a line saying so in the file's place; Claude tells you, and
the chat marks the file.

## 3. Photos are made smaller

A phone photo is 12 megapixels and 4 to 8 MB; Claude scales every picture
down to 2576 pixels on its long side (1568 on models before Claude 4.7)
before it looks. So the panel does that first, in the browser: about half
a megabyte goes over mobile data instead, nothing Claude would have seen
is lost, and the photo's place and device data stay behind.

Settings › Composer › Attachments turns this off or picks 1568 px; **+ ›
Photos in full size** is the same switch. For fine print send the photo
in full and ask Claude to crop: it can cut a region out of the original
with its tools and read that. Screenshots that are small already go as
they are, and a GIF keeps its frames.

## 4. Files: what is kept, and for how long

**Files** in the sidebar lists everything attached and still on the hub:
the picture or the kind, the session it came from, the size, whether a
machine ever fetched it, and when the hub drops it.

- A file is dropped 30 days after its upload (`retention_days`).
- All files together may take 5 GB (`quota_mb`); when the store is full,
  the oldest ones go first.
- **Keep** pins a file past both; **Delete** takes it away now.

Dropping a file loses little: once Claude read a picture it is part of
that conversation, on the session's machine. A session's machine keeps
the files it fetched for 7 days after their last use
(`[attachments] keep_days` in its collector's configuration).

## 5. How it works

In order:

- The panel uploads the file to the hub (`POST /api/v1/attachments`,
  the body streamed to disk under `attachments.dir`) and gets an id.
- The prompt goes out with a marker per file, `[ccdash-attachment <id>]`.
- The collector of the session's machine sees the marker when it
  delivers the prompt (through the channel, as a new session's first
  prompt, or as a note with the next prompt), fetches the file with its
  machine's token into `~/.local/state/ccdash/attachments/<id>/`, and
  puts the line with that path in the marker's place.
- Claude reads the file. Reading outside a session's own directory
  needs a permission: the Home Manager module adds the rule for that
  directory (`services.ccdash.claudeCode.attachments.enable`), and
  sessions started from the panel carry it in their settings.

A session that moves to another machine gets its later attachments
there; the hub's copy is the one that counts.

## 6. Turning it off, and the bounds

On the hub (`services.ccdash-hub.attachments` in the NixOS module, or
`[attachments]` in its configuration):

| Key              | Default                             | Is                                                |
| ---------------- | ----------------------------------- | ------------------------------------------------- |
| `enable`         | `true`                              | off: no attaching in the panel, no Files area     |
| `dir`            | `attachments/` next to the database | where the files are                               |
| `max_image_mb`   | `25`                                | the largest picture                               |
| `max_audio_mb`   | `50`                                | the largest sound recording                       |
| `max_video_mb`   | `300`                               | the largest video                                 |
| `max_file_mb`    | `50`                                | the largest file of another kind                  |
| `quota_mb`       | `5120`                              | all files together; `0`: no quota                 |
| `retention_days` | `30`                                | when an unkept file is dropped; `0`: never by age |

The NixOS module sizes nginx's body limit for the upload from the largest
of the four caps.

On a workstation (`services.ccdash.settings.attachments`, or
`[attachments]` in the collector's configuration):

| Key         | Default                                    | Is                                                    |
| ----------- | ------------------------------------------ | ----------------------------------------------------- |
| `enable`    | `true`                                     | off: this machine fetches nothing, the panel says so  |
| `dir`       | `attachments/` in ccdash's state directory | where fetched files go                                |
| `keep_days` | `7`                                        | a fetched file unused this long is dropped; `0`: kept |

A machine fetches only when its collector has a token for the hub
(`hub.token_file`).

## 7. Dictation

The microphone in a message field (`Ctrl Shift D`, or **+ › Dictate**)
records; **Done** sends the recording to a machine that puts it into
words, and the words land at the cursor for you to read over, correct and
send. Nothing is sent by itself. **Cancel** throws the recording away. A
recording stops by itself after ten minutes, and it is not kept: the hub
drops it once the words are back.

The words are made on a workstation, by
[whisper.cpp](https://github.com/ggml-org/whisper.cpp), on your own
hardware: nothing goes to a service. One machine has to offer it:

```nix
services.ccdash.transcribe = {
  enable = true;
  language = "auto"; # or "cs", "en": steadier on short sentences
};
```

That fetches the `small` model (466 MB, many languages; a sentence takes
a few seconds on a desktop's processor) and sets `[transcribe] command`.
`transcribe.model` takes another model file, `transcribe.extraArgs` more
arguments for `whisper-cli`; without the module, set the command
yourself: it gets the recording's path as `{file}` and prints the words.

The panel asks the session's own machine when it can, else any connected
one that can; Settings › Composer › Dictation names one for good. The
microphone needs the panel opened over https (or on localhost), and the
browser asks once for it.

## 8. Recordings and videos

Claude cannot hear or watch, so the session's machine makes something it
can read, once, when the prompt is delivered:

- of a **video**, still frames spread evenly from its start to its end
  (12 by default, at most 1568 pixels on the long side), with
  `services.ccdash.video.enable` (ffmpeg);
- of a **video or a sound recording**, the words said in it, in a text
  file, with `services.ccdash.transcribe.enable`.

The prompt names both under the file's own line, and Claude reads the
frames it needs:

```text
[Attached video, which you cannot watch: /…/attachments/3e2b…/clip.mp4]
[Frames of 3e2b…: 12 stills, one every 3.5 s from its start to its end, are in /…/3e2b…/derived/frames/ as 001.jpg to 012.jpg. Read the ones you need.]
[Transcript of 3e2b…: what is said in it is in /…/3e2b…/derived/transcript.txt]
```

The chat shows the video or the recording with a player and a line with
what Claude got. When a machine has neither tool, or one fails, the
prompt says that instead, so Claude does not guess at a file it cannot
open. A long video takes a while to cut and to put into words: the prompt
goes out when that is done, and the chat shows it as sent meanwhile.

| Key                       | Default      | Is                                                    |
| ------------------------- | ------------ | ----------------------------------------------------- |
| `transcribe.command`      | `[]` (off)   | prints what is said in `{file}`                       |
| `transcribe.timeout_secs` | `300`        | the command is stopped after this long                |
| `transcribe.attachments`  | `true`       | attached recordings and videos get their words too    |
| `video.ffmpeg`            | `""` (off)   | the `ffmpeg` to cut frames with (`ffprobe` beside it) |
| `video.frames`, `.edge`   | `12`, `1568` | how many frames, and their longest side in pixels     |
| `video.timeout_secs`      | `300`        | cutting is stopped after this long                    |

## 9. Not yet

- Recording a voice message or a video inside the panel as an attachment:
  the phone's own camera and recorder do it through **Photo or file**.
- Attaching a file again from Files, and the phone's Share sheet.
