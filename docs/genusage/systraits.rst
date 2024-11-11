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

.. _systraits:

System Traits
=============

.. note::

    Definition and description of system traits and their purpose.

Traits are essentially static attributes of a minion. They can be a literally anything
in a form of key/value. There are different kinds of traits:

**Common**

   Common traits are given to each minion automatically. They are typical system
   information and anything else that can be commonly fetched out of the "box": OS info,
   kernel version, memory size, network settings, hostnames, machine Id etc.

**Custom**

   Custom traits are static data that set explicity onto a minion. Any data in
   key/value form. They are usually various labels, rack number, physical floor,
   Asset Tag, serial number etc.

**Dynamic**

   Dynamic traits are custom functions, where data obtained by relevant modules.
   essentially, they are just like normal modules, except the resulting data is stored as
   a criterion by which a specific minion is targeted. For example, *"memory less than X"*,
   or *"runs process Y"* etc.

Listing Traits
--------------

(TBD) How to see traits of minions.

Using Traits in a Model
-----------------------

(TBD) Targeting distributed entities by traits etc.

Static Minion Traits
--------------------

(TBD) As a part of minion configuration.

Populating Traits
-----------------

(TBD) How to push traits to all minions and fetch theirs