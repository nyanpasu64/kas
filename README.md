My GUI library
==========


Goals
------

Rust currently has a smattering of GUI libraries, but none currently offer the
full complement of features which really show off the strengths of the Rust
language:

-   **safe**: GUIs are complex, high-level constructions; they really should
    make it easy to write memory- and thread-safe code
-   **easy**: while the behaviour expressed by GUIs is complex, the ideas behind
    them are usually not; it should therefore be easy to write typical GUIs
-   **flexible building blocks**: the Rust language has succeeded in keeping the
    language specification *moderately* simple while buliding a rich library
    on top of this; a Rust GUI library should do the same
-   **static type model**: Rust has succeeded in allowing most types to have
    compile-time known size and static linkage; not only does this avoid
    unnecessary run-time memory allocation, but also enables a *lot*
    of compile-time optimisation

Note that certain trade-offs must be made to allow the above goals; in
particular this means the library will not be easy to use via FFI (e.g. from C):

-   **complex types**: many widget types are complex or outright unnameable
-   **heavy use of macros**: while users *should* be able to implement
    functionality directly, this may be verbose and monotonous; macros can be
    used to construct the necessary implementations more succinctly


Widgets
--------

Core building block: widgets. These have: internal data, user data, event
handling, sub-widget positioning, ...

### Facilities

Internal data: data for internal use by the lib (e.g. position offset).
TBD how users deal with this when writing their own types.

User data: widgets may carry user-defined data specified via a constructor and
accessible during event handling.

Event handling: use a series of handlers like `handle_new_location`,
`handle_mouse_click`, `handle_key_entry`, etc. All should have a default
implementation from the trait, usually doing nothing. User can implement as
necessary. Event handlers can access user data and call functions on child
widgets.

User-defined return value: all event handlers return a result, often `None`
but potentially other predefined things (e.g. `Resize`, `Close`), or user
defined values. The user-defined type must allow construction from the core
event type (via `From`) and optional coercion back to it (via `TryFrom`).

Handling event return values: the user should be able to catch return values
from sub-widget event handling in any parent widget (TBD). This handler can
access user data and call functions on child-widgets.

Sub-widget positioning: this is only something that needs to happen on
construction and on adjustment; at other times cached offsets may be used.
Implement a "layout" trait with functions for getting sizing and calculating
offsets for sub-widgets (recursively), etc.

### Construction

A widget will normally be a struct with a set of associated trait
implementations. The struct's fields will hold internal data, user data and
sub-widgets. Implementations may use these fields directly.

### Built-in widgets

The library will define many built-in widgets. These will likely be templated
structs. Users should be able to pass user data and callbacks to handle
common actions (e.g. button press or text field change). Some will allow
sub-widgets.

Examples:

```rust
let window = Window::new(
    "Simple Window",    // title
    (),     // user data
    Button::new(    // single sub-widget
        "Close",    // button text
        || Response::Close  // closure called on press, returning Close result
    ));
```

### User-defined widgets (low level)

Users may define their own widgets. This will be useful for handling event
actions as well as building custom types of widget.

Although sub-widgets will often have a single compile-time-known sub-type,
type templating will often be useful (to avoid having to describe complex types)
and in many cases even necessary (because closure types are not nameable).

```rust
struct MyWidget<A, B> {
    // TODO: data
    text: A,
    button: B,
}
```

Macros will be available to facilitate implementation of required traits.

```rust
impl_layout!(MyWidget,
    vlist(
        text,
        button
    )
)
```

### User-defined widgets (high level)

TBD: a macro to construct complex new widgets (all the above, covering at least
the common cases).
