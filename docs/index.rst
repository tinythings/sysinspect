Welcome to the Project Sysinspect!
==================================

.. note::
   This documentation covers **Sysinspect** — the multi-solution to many things.

Welcome to **Sysinspect**: originally conceived as an engine for anomaly detection and root cause analysis. The name
itself is a portmanteau of "system" and "inspection", which should give you a fair idea of its intent.

**Sysinspect** began life as a generic engine designed to monitor system consistency, using a formal Model Description
of the system's architecture. Over time, it has grown into a collection of tools and libraries for system introspection,
configuration management, anomaly detection, root cause analysis, and automated remediation. The project is shaped by
the needs and contributions of its users, and is intended to be both hackable and extensible.

With **Sysinspect**, you can:

- Examine and introspect arbitrary systems, provided you have a Model Description of their architecture.
- Detect anomalies and perform root cause analysis using a combination of rules, heuristics, and data-driven methods.
- Track configuration drift and check compliance against a known-good state.
- Automate remediation actions, either as one-offs or as part of a workflow.
- Extend the system with your own modules, handlers, and integrations.
- Run it on Android, embedded Linux environments with questionable tooling (or none at all), or traditional server environments.

If you enjoy tinkering with system internals, building automation, or just poking around to see how things work,
Sysinspect is meant to be a toolkit you can adapt and extend. Contributions, bug reports, and wild ideas are all
welcome—see the section on contributing for how to get involved.

.. toctree::
   :maxdepth: 1
   :caption: Documentation

   global_config
   genusage/overview
   modeldescr/overview
   moddev/overview
   moddescr/overview
   evthandlers/overview
   uix/ui
   apidoc/overview

.. toctree::
   :maxdepth: 1
   :caption: Tutorials

   tutorial/cfgmgmt_tutor
   tutorial/action_chain_tutor
   tutorial/module_management


Licence
-------

**Sysinspect** is distributed under Apache 2.0 Licence.

Limitations
-----------

Currently **Sysinspect** is an experimental concept.

Contributing
------------

Best way to make progress is to open an issue (great 👍) or submit a
Pull Request (awesome 🤩) on the GitHub.

And just in case you don't know how to implement it in Rust, but you
still want something cool to happen, then please fill-in an issue
using issue tracker.
