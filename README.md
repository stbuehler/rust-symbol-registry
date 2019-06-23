# `symbol-registry`

A crate providing sharing of non-mutable strings ("symbols") through
registries; each registry will store a certain value only once.

A registry will also cleanup (i.e. free) a string if it isn't used by
any `Symbol` anymore.

Symbols can also be created standalone.

## Implementation

Each `Symbol` has a (strong) reference count; when it reaches `0` the
symbol will be removed from the registry, unless the symbol got cloned
from the registry in the meantime again.

The registry itself keeps no reference on the symbol (one might view it
as a "weak" reference), and the symbols keep weak reference of the
registry.

The string data will be stored directly after the metadata of it (i.e.
the reference count, the registry reference and the length of the
string).
