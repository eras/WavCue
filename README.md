# WavCue

Author: Erkki Seppälä <erkki.seppala@vincit.fi>
License: [MIT](LICENSE.MIT)

WavCue is a tool for converting cue data from WAV files in a CSV format
[SonicVisualizer](https://sonicvisualiser.org/) can import. Of
particular interest are the ones produced by Zoom H1n which generates
them with its mark function. One mark is corresponds to one cue entry.

In addition to reading the `cue` chunk the tool also reads the
Broadcast Audio Extension (`brex`), enabling the mark labels in the
CSV to also indicate the correct time of day.

Example of the data produced by this tool:

```sh
% wav-cue ZOOM0001.WAV > ZOOM0001.csv
% cat ZOOM0001.csv
2.773,Mark 1 12:23:42
63.045,Mark 2 12:24:43
```

You can use the function File/Import Annotation Layer (shortcut `G`)
to import it into SonicVisualizer.

# Downloading

Get your binaries for Linux, Mac and Windows from the Releases.
