# Changelog

Currently, the changelog is built locally. It will be moved to CI once labels stabilize.

For now, a bit of preparation is required before you can run the script:
- fetch the srtool digests
- store them under the `digests` folder as `<chain>-srtool-digest.json`
- ensure the `.env` file is up to date with correct information. See below for an example

The content of the release notes is generated from the template files under the `scripts/ci/changelog/templates` folder. For readability and maintenance, the template is split into several small snippets.

Run:
```
./bin/changelog <ref_until> [<ref_since>]
```

For instance:
```
./bin/changelog v0.9.18
```

A file called `release-notes.md` will be generated and can be used for the release.

## ENV

You may use the following ENV for testing:

```
RUSTC_NIGHTLY="rustc 1.57.0-nightly (51e514c0f 2021-09-12)"
PRE_RELEASE=true
HIDE_SRTOOL_SHELL=true
DEBUG=1
```
## Considered labels

The following list will likely evolve over time and it will be hard to keep it in sync.
In any case, if you want to find all the labels that are used, search for `meta` in the templates.
Currently, the considered labels are:

- Priority: C<N> labels
- Audit: D<N> labels
- E4 => new host function
- E2 => database migration
- B0 => silent, not showing up
- B1-releasenotes (misc unless other labels)
- B5-client (client changes)
- B7-runtimenoteworthy (runtime changes)
- T6-XCM

Note that labels with the same letter are mutually exclusive.
A PR should not have both `B0` and `B5`, or both `C1` and `C9`. In case of conflicts, the template will
decide which label will be considered.

