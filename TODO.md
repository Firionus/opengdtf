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
- [ ] Test XML serialization in new arch by doind a roundtrip after parsing
- [x] Switch to derived serialization/deserialization on Gdtf and wrappers for expected.toml
- [ ] Write out problems or errors occuring while testing to a separate file and the console, this makes it easier to spot difficult examples