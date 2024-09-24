# Model checkbook

Model itself is a graph. But graph has no entry or no exit.
It just explains a system components and their relations.

To start checking a system, one needs a general index, where
to start and what top-level components are needed at the beginning.

Checkbook is just a list of top entities to be verified. Not
all entities needs to be verified as some of them might be removed
from the checkbook.

```yaml
checkbook:
  - logging
  - general-network
```

## Flow

The flow works the following way:

1. Checkbook provides only "entrance" spots where Syspect needs
to start investigations.

2. An entity, if is a collection of other entities (a group) will
go into the details. Otherwise, if an entity is the end leven with
facts, will call an actual check.

3. If a single entity is hit (i.e. not group entity), then facts
needs to be verified. **Actions** are triggers to verify the validity
of those facts. Actions are triggered by the binding to the entities
by group bindings or the same ID.

4. Iterating over facts collection, a **constraint** is defining a logic
how to deal with it. For example, facts contains two or three routes,
but only one is available. A constraint explains in a declarative
fashion how to accept it.
