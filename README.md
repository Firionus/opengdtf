# opengdtf

[![Project Status: WIP – Initial development is in progress, but there has not yet been a stable, usable release suitable for the public.](https://www.repostatus.org/badges/latest/wip.svg)](https://www.repostatus.org/#wip)


A starting point to build a useful open-source [GDTF](https://gdtf-share.com/)
libary in Rust.  
GDTF is a standardized fixture format for entertainment lighting.

:construction: Early initial development, not usable yet :construction:  
If you are interested in helping out, open a thread in the Discussions and say
hi :wave:

## Short-term goals

- Parse some fundamental data from GDTF files
- Show it can be bound to other languages (e.g. try a JS binding with
  [Neon](https://github.com/neon-bindings/neon) for
  [OFL](https://github.com/OpenLightingProject/open-fixture-library))
- Documentation around GDTF and how this library handles it to help other
  developers chime in
- Build community support

ToDo's are kept in [Issues](https://github.com/Firionus/opengdtf/issues).

## Principles

**Don't just parse GDTF**

Provide a useful higher-level API that resembles the fixture model in a useful
way. 

**Suck out as much information as possible**

Always return a valid result, even if some parts are fixed up or missing. Use
mitigations (defaults, renames, ...) for unexpected conditions when possible or
omit a part if mitigation is impossible or too hard.  
In any case, indicate problems and actions taken in a Problem Vector.  
Panics are usually considered a bug, because then no valid result is returned.

**Move faster**

Value developer time. 

### Rationale for Principles

- GDTF files in themselves are barely useful, for example you can't even know
  how many DMX channels a mode has by looking at the file. So they must be
  processed before being presented to a library user. 
- The GDTF consortium does not have a culture of ensuring validity and a parser
  must be lenient to be useful. 
- Noone, including me, seems to have much time left over for open source GDTF

## Non-goals

- Serialize GDTF XML. This library provides a high level representation that can't be translated back to GDTF losslessly. GDTF creation is therefore out of scope for now, and might later be more easy to tackle with a wrapper around an XML mutable DOM library like minidom or similar. 

## Why Rust?

Rust is my language of choice for this project, due to its interoperability with
other languages and safety.  

Interoperability is needed because creative lighting people want to work in many
different languages but the community is not big enough to create GDTF
implementations in every language. Safety is important because the environments
in which we create stage lighting are already stressful, and software should not
be a burden by being unreliable.

Other languages without a runtime, like C/C++, are interoperable but not very
safe. Safe languages on the other hand often have a runtime that makes language
interoperability unfeasible. This is why I think Rust is a good choice for a
GDTF library.

## How to Develop

see [CONTRIBUTING.md](CONTRIBUTING.md)

## Other Links and Projects

- [gdtf_parser](https://github.com/michaelhugi/gdtf_parser): More complete
  parser than opengdtf, but aims for a lower level of abstraction and less
  forgiving parser
- [mvrdevelopment/spec](https://github.com/mvrdevelopment/spec): The official
  GDTF and MVR specification. Also the place to file issues against the
  specification. 
