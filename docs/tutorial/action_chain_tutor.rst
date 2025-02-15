.. raw:: html

   <style type="text/css">
     span.underlined {
       text-decoration: underline;
     }
     span.bolditalic {
       font-weight: bold;
       font-style: italic;
     }
   </style>

.. role:: u
   :class: underlined

.. role:: bi
   :class: bolditalic

.. _actionchain_tutorial:

Action Chains
=============

.. note::
   This documentation explains action chains and task ordering in Configuration Management mode.

   It is assumed, you went through the :ref:`cfgmgmt_tutorial` tutorial and you understand
   how it generally works.

Overview
--------

An *"Action Chain"* is actually just a series of calls to modules, organised in specific way
that would meet required demands.

In various configuration management solutions, one may alter a scenario, so that states are
executed precisely as expected. Typically, it is used to ensure that a state preformed
exactly how it was expected to, as well as to ensure that a state only runs under certain
conditions. For example, other systems use ``onlyif`` or ``unless``, also ``require`` or
``before`` or ``after`` etc. This leads to the following chaos:

    1. **A** starts
    2. **B** requires **C**
    3. **C** requires **A** and **E**
    4. **D** starts
    5. **E** requires **B**

And this is only 5 (five) actions. Imagine you have 200 of those, randomly requiring each other.
Add Jinja conditions of blocks. Who would enjoy that?

This kind of approache unnesessarily overcomplicates everything. If your model grows
and is big enough, it will be to :bi:`nearly undebuggable` to manually find out which action
goes after which. For that reason, Sysinspect does not use manual flags to reorder execution.
Instead, each action is called in the precise order as it is defined in the YAML of the model.

.. important::

    **Sysinspect** is executing actions exactly in the specified order, as it is defined in the model.

In Sysinspect, one would do two things. *First*, write the above the following straightforward way:

    1. **A**
    2. **E**
    3. **C**
    4. **B**
    5. **D**

And a *second*, it would became clear that in the first example there is a circular dependency loop,
which you did not spotted by now: **E** requires itself to start!

Requisites
----------

Currently there are two requisites supported for conditional action process: if something is true
and something is false.

.. warning::

    In order to let requisites work, a constraint has to be defined for the action, and executed
    :bi:`prior` their evaluation.

``if-true``
===========

This requisite specifies that the current action can start only if a list of **all specified constraints**
returns positive (success).

Example:

.. code-block:: yaml

    actions:
      install-firefox:
        descr: Install Firefox browser
        ...
        if-true:
          - install-firefox

    constraints:
      install-firefox:
        descr: Check if Firefox was installed
        all:
          $:
            - fact: changed
              equals: true

``if-false``
============

This requisite specifies that the current action can start only if a list of **all specified constraints**
returns negative (fails).

.. code-block:: yaml

    actions:
      remove-firefox:
        descr: Remove/uninstall Firefox browser
        ...
        if-false:
          - firefox-installed

    constraints:
      firefox-installed:
        descr: Check if Firefox is currently installed
        all:
          $:
            - fact: absent
              equals: false
