# opengdtf

[![Project Status: WIP – Initial development is in progress, but there has not yet been a stable, usable release suitable for the public.](https://www.repostatus.org/badges/latest/wip.svg)](https://www.repostatus.org/#wip)


A starting point to build a useful open-source [GDTF](https://gdtf-share.com/)
libary in Rust.

:construction: Early initial development, not usable yet :construction:  
If you are interested in helping out, open a thread in the Discussions and say
hi :wave:

## Short-term goals

- Parse some fundamental data from GDTF files
- Show it can be bound to other languages (e.g. try a JS binding with [Neon](https://github.com/neon-bindings/neon) for [OFL](https://github.com/OpenLightingProject/open-fixture-library))
- Documentation around GDTF and how this library handles it to help other developers chime in
- Build community support

ToDo's are kept in [Issues](https://github.com/Firionus/opengdtf/issues).

## Principles

**Don't just parse GDTF**

Provide a useful higher-level API that resembles the fixture model in a useful
way. 

**Never panic** 

Indicate Problems in an Error Vector instead.

**Suck out as much information as possible**

Always return a result, even if some parts are broken.  

**Indicate when things are broken**

If parts of the result are broken, it must be indicated in the error vector.

It is okay to replace broken things with sensible defaults, but this must be
indicated in the error vector and leaving out broken things should be preferred
if this does not clash with the preceding principle. 

**Move fast and break things**

Value developer time. If a value does not have to be read programmatically by
the user, it is okay to just return the string from the file instead of an enum.
However, it should be validated and errors indicated in the error vector. 

### Rationale for Principles

- GDTF files in themselves are barely useful, for example you can't even know
  how many DMX channels a mode has by looking at the file. So they must be
  processed before being presented to a library user. 
- The GDTF consortium does not have a culture of ensuring validity and a parser
  must be lenient to be useful. 
- Noone, including me, seems to have much time left over for open source GDTF

## Long-term goals

- Serialize GDTF XML, e.g. with quick-xml