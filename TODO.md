This file tracks my progress through trying to write this again from the ground up. 

The principles from the README remain, the approach changes inspired by #4. The
main problem so far was a huge chunk of code for parsing with so much error
handling that you could barely see what's going on.

The architecture will look like this:
- GDTF files are parsed to an intermediate struct that only validates values
  locally, not in their container or links. Things in the intermediate struct
  feature derived builders for easier push-parsing.
- This intermediate is then used to iteratively construct a Gdtf struct with a
  public validating API that would also be used by an application to build a
  GDTF from scratch
- This validated Gdtf can be converted to the intermediate struct, which is then serialized

I'll try to write the serialization in quick-xml with serde. That would also
allow deriving the parsing but it would not work with the robust error handling
I'm looking for (can only fail or succeed, not have side effects which I want).
Therefore, I'll hand-write the parsing with quick-xml and builders for the
intermediate struct.

- [x] Test parsing in new arch
- [x] Test XML serialization in new arch by doind a roundtrip after parsing
- [x] Switch to derived serialization/deserialization on Gdtf and wrappers for expected.toml
- [x] Write out problems or errors occuring while testing to a separate file and/or the console, this makes it easier to spot difficult examples -> added to update_expected.rs

## Geometries Design

- Geometries form a tree
- References can "instantiate" a "template" in the tree, linking the template and the reference geometry
- Geometries reference models
- DmxModes and channels reference Geometries
- and so on...
- How to handle all of this referencing?
  - I want a non-panicking API, so petgraph is out
  - I want something rock-solid, not something with dangling references
  - Performance is not important
- Idea: Just use straight forwards trees with vecs of vecs
  - Just use the gdtf keys for lookup, which mostly are strings
- [ ] refactor module names and exports while taking care about duplicate names in low_level and high_level
- [ ] review when we parse things to Name type, whether we can just use the "fixed version" instead of a default or other less good error handling
- [ ] remove all unwrap from tests and replace with anyhow::Result return type and question mark, so much shorter