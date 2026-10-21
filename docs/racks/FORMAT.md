# The `.cwr` custom rack file

A `.cwr` file holds one custom rack: the rack instrument, its layer
instruments with every setting, their effect chains (Splits included) and
the samples they play. ChipWave Studio keeps them in the **Racks** folder
inside its settings folder (`<config>/Racks`). Browser › My Racks lists that
folder.

## Container

A ZIP archive:

| entry | compression | contents |
|---|---|---|
| `mimetype` | stored, first | `application/x-chipwave-rack` |
| `rack.toml` | deflated | everything except sample audio (below) |
| `samples/NN.wav` | stored | 32-bit float WAV of instrument `NN`'s sample, when it is a sample instrument |

## rack.toml

```toml
format = "chipwave-rack"
format_version = 1
app = "ChipWave Studio 0.2.6"        # informational

[rack]
name = "Glass Keys"
description = "bright keys with a soft pad underneath"
sound_type = "Keys"                  # free text; the Save dialog offers a list
tags = ["bright", "glass"]
created = "2026-10-21"               # UTC dates
updated = "2026-10-21"

[[instruments]]                      # entry 0: the rack
# ... the same instrument fields a .cwt song uses, kind = Rack with
# layers = [{ slot = 1, on = true, level_db = 0.0, pan = 0.0,
#             transpose = 0, key_lo = 1, key_hi = 120 }, ...]

[[instruments]]                      # entries 1..n: the layers' instruments
# ...

[[plugins]]                          # plugin effects used anywhere (for the
format = "clap"                      # missing-plugin warning, without
id = "com.example.verb"              # reading every instrument)
name = "Example Verb"
vendor = "Example"
used_by = "layer B › effects"
```

In the file, a layer's `slot` is an index into `instruments`. When the rack is
loaded into a song, its instruments get new slots and the layers are
renumbered to match. A sample instrument's `file` names its `samples/NN.wav`
entry.

## Rules checked before anything changes

A file is refused, with one sentence saying why and with nothing in the song
changed, when:

- `format_version` is newer than this ChipWave reads ("…was made by a newer
  ChipWave (rack format N). Please update to load it."). The version is read
  first, before the rest is parsed.
- the archive or `rack.toml` can't be read ("…is damaged").
- it holds no instruments, or the first one isn't a rack.
- the rack has fewer than 1 or more than 4 layers, a layer points outside the
  file, two layers share an instrument, a layer is itself a rack, or an
  instrument isn't used by any layer.
- a Split sits more than 2 deep, or a top-level Split has no paths or more than 4.

Every entry is read with a size cap (256 MB), so a broken or hostile file
can't exhaust memory.

## Saving, updating and deleting

- **Save** writes a temporary file beside the target and renames it into place,
  so a crash never leaves half a rack. A plain instrument is saved as a rack
  with one layer. Plugin instruments can't be saved as racks, because they
  can't play as a layer yet.
- When a rack with the same name already exists, the Save dialog asks you to
  choose **Update existing** or **Save as new**. It never overwrites a file
  without asking.
- **Update** (Save dialog or My Racks › Update from track's sound) copies the
  old file into `Racks/.backup` first and keeps the newest 3 backups per rack.
  The rack's name, tags and created date stay.
- **Delete** asks first and then moves the file to `Racks/Deleted`. It is not
  erased.

## Versioning

`format_version` goes up only when an older ChipWave would load a newer file
wrongly. Fields that can be missing have defaults, so new optional fields
don't need a new version.
