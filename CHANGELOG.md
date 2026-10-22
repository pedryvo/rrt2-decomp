# Changelog

Entry format: `- <Kind>: text`, where Kind is Feature, Improved, Change, Fix, Docs or RE (reverse-engineering finding). Append `(original bug)` to fixes of bugs present in the original game.

## Unreleased

- Feature: PK2 archive reader with filename-hash lookup and the `rt2-extract` tool.
- Feature: LNG string-table parser (last definition wins, named `;t<NAME>_0` tables).
- RE: PK2 filename hash, archive load order and precedence recovered from the executable.
- Docs: development plan, file-format specs for PK2 and LNG, project README.
- RE: PK2 contents classified: LAD sprite format (8-bit indexed, per-row RLE, layered), palette banks, RGB palettes, IMB layouts and raw PCM audio.
- Docs: game rules, formulas and data tables extracted from the manual, strategy guide and editor notes (docs/reference/manual-rules.md).
