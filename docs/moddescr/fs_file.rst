``fs.file``
===========

.. note::

    This document describes ``fs.file`` module usage.

Synopsis
--------

This plugin is intended to perform basic CRUD operations on files
in the filesystem. You can create (touch) an empty file,
fill it with the content from the remote data or copy a local
file, delete etc.

Usage
-----

The following options are available:

  ``create``
    Create an empty file with possible content (see args)

  ``delete``
    Delete a specified file


The following arguments to the options are available:

  ``name`` (type: string, required)
    A target filename

  ``mode`` (type: string)
      Can be one of ``strict`` and ``easy``.

      ``strict``
        the module will always return non-zero error
        code if the state wasn't changed.

      ``easy`` (default)
        the module will return error code zero

  ``pull`` (type: string)
      If starts with schema ``file://``, then it means that the
      file resoure is local. Otherwise it is a filename, served
      on the master's data fileserver's root in order to use it
      as a content.


Examples
--------

This is the basic usage. Below is the example how to pull a file from a master:

.. code-block:: json

    {
      "opts": ["create"],
      "arguments": {
        "name": "/etc/group",
        "pull": "/standard/group"
      }
    }

This is an example of copying a local file:

.. code-block:: json

      {
        "opts": ["create"],
        "arguments": {
          "name": "/backup/etc/group",
          "pull": "file:///etc/group"
        }
      }



Returning Data
--------------

Returns just a regular text of the command STDOUT. If fill specified:

.. code-block:: json

      {
        "message": "Content of /etc/group updated",
        "retcode": 0,
        "data": {
          "changed": true
        }
      }
