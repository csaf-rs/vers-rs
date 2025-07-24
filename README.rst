*This document was taken from https://github.com/package-url/purl-spec/blob/main/VERSION-RANGE-SPEC.rst*

Version range specifier
------------------------

A version range specifier (aka. "vers") is a URI string using the ``vers``
URI-scheme with this syntax::

    vers:<versioning-scheme>/<version-constraint>|<version-constraint>|...

For example, to define a set of versions that contains either version ``1.2.3``,
or any versions greater than or equal to ``2.0.0`` but less than ``5.0.0`` using
the ``node-semver`` versioning scheme used with the ``npm`` Package URL type,
the version range specifier will be::

    vers:npm/1.2.3|>=2.0.0|<5.0.0

``vers`` is the URI-scheme and is an acronym for "VErsion Range Specifier". It
has been selected because it is short, obviously about version and available
for a future formal URI-scheme registration at IANA.

The pipe "|" is used as a simple separator between ``<version-constraint>``.
Each ``<version-constraint>`` in this pipe-separated list contains a comparator
and a version::

    <comparator:version>

This list of ``<version-constraint>`` are signposts in the version timeline of
a package that specify version intervals.

A ``<version>`` satisfies a version range specifier if it is contained within
any of the intervals defined by these ``<version-constraint>``.


Using version range specifiers
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

``vers`` primary usage is to test if a version is within a range.

An version is within a version range if falls in any of the intervals defined
by a range. Otherwise, the version is outside of the version range.

Some important usages derived from this include:

- **Resolving a version range specifier to a list of concrete versions.**
  In this case, the input is one or more known versions of a package. Each
  version is then tested to check if it lies within or outside the range. For
  example, given a vulnerability and the ``vers`` describing the vulnerable
  versions of a package, this process is used to determine if an existing
  package version is vulnerable.

- **Selecting one of several versions that are within a range.**
  In this case, given several versions that are within a range and several
  packages that express package dependencies qualified by a version range,
  a package management tools will determine and select the set of package
  versions that satisfy all the version ranges constraints of all dependencies.
  This usually requires deploying heuristics and algorithms (possibly complex
  such as sat solvers) that are ecosystem- and tool-specific and outside of the
  scope for this specification; yet ``vers`` could be used in tandem with
  ``purl`` to provide an input to this dependencies resolution process.


Examples
~~~~~~~~~

A single version in an npm package dependency:

- originally seen as a dependency on version "1.2.3" in a package.json manifest
- the version range spec is: ``vers:npm/1.2.3``


A list of versions, enumerated:

- ``vers:pypi/0.0.0|0.0.1|0.0.2|0.0.3|1.0|2.0pre1``


A complex statement about a vulnerability in a "maven" package that affects
multiple branches each with their own fixed versions at 
https://repo1.maven.org/maven2/org/apache/tomee/apache-tomee/ 
Note how the constraints are sorted:


- "affects Apache TomEE 8.0.0-M1 - 8.0.1, Apache TomEE 7.1.0 - 7.1.2,
  Apache TomEE 7.0.0-M1 - 7.0.7, Apache TomEE 1.0.0-beta1 - 1.7.5."

- a normalized version range spec is:
  ``vers:maven/>=1.0.0-beta1|<=1.7.5|>=7.0.0-M1|<=7.0.7|>=7.1.0|<=7.1.2|>=8.0.0-M1|<=8.0.1``

- alternatively, four ``vers`` express the same range, using one ``vers`` for
  each vulnerable "branches": 
  - ``vers:tomee/>=1.0.0-beta1|<=1.7.5``
  - ``vers:tomee/>=7.0.0-M1|<=7.0.7``
  - ``vers:tomee/>=7.1.0|<=7.1.2``
  - ``vers:tomee/>=8.0.0-M1|<=8.0.1``

Conversing RubyGems custom syntax for dependency on gem. Note how the
pessimistic version constraint is expanded:

- ``'library', '~> 2.2.0', '!= 2.2.1'``
- the version range spec is: ``vers:gem/>=2.2.0|!= 2.2.1|<2.3.0``


URI scheme
~~~~~~~~~~

The ``vers`` URI scheme is  an acronym for "VErsion Range Specifier".
It has been selected because it is short, obviously about version and available
for a future formal registration for this URI-scheme at the IANA registry.

The URI scheme is followed by a colon ":".


``<versioning-scheme>``
~~~~~~~~~~~~~~~~~~~~~~~

The ``<versioning-scheme>`` (such as ``npm``, ``deb``, etc.) determines:

- the specific notation and conventions used for a version string encoded in
  this scheme. Versioning schemes often specify a version segments separator and
  the meaning of each version segments, such as [major.minor.patch] in semver.

- how two versions are compared as greater or lesser to determine if a version
  is within or outside a range.

- how a versioning scheme-specific range notation can be transformed in the
  ``vers`` simplified notation defined here.

By convention the versioning scheme **should** be the same as the ``Package URL``
package type for a given package ecosystem. It is OK to have other schemes
beyond the purl type. A scheme could be specific to a single package name.

The ``<versioning-scheme>`` is followed by a slash "/".


``<version-constraint>``
~~~~~~~~~~~~~~~~~~~~~~~~

After the ``<versioning-scheme>`` and "/" there are one or more
``<version-constraint>`` separated by a pipe "|". The pipe "|" has no special
meaning beside being a separator.

Each  ``<version-constraint>`` of this list is either a single ``<version>`` as
in ``1.2.3`` for example or the combination of a ``<comparator>`` and a ``<version>`` as in
``>=2.0.0`` using this syntax::

    <comparator><version>

A single version that means that a version equal to this version satisfies the
range spec. Equality is based on the equality of two normalized version strings
according to their versioning scheme. For most schemes, this is a simple string
equality. But schemes can specify normalization and rules for equality such as
``pypi`` with PEP440. 


The special star "*" comparator matches any version. It must be used **alone**
exclusive of any other constraint and must not be followed by a version. For
example "vers:deb/\*" represent all the versions of a Debian package. This
includes past, current and possible future versions.


Otherwise, the ``<comparator>`` is one of these comparison operators:

- "!=": Version exclusion or inequality comparator. This means a version must
  not be equal to the provided version that must be excluded from the range.
  For example: "!=1.2.3" means that version "1.2.3" is excluded.

- "<", "<=": Lesser than or lesser-or-equal version comparators point to all
  versions less than or equal to the provided version.
  For example "<=1.2.3" means less than or equal to "1.2.3".

- ">", ">=": Greater than or greater-or-equal version comparators point to
  all versions greater than or equal to the provided version.
  For example ">=1.2.3" means greater than or equal to "1.2.3".


The ``<versioning-scheme>`` defines:

- how to compare two version strings using these comparators, and

- the structure of a version string such as "1.2.3" if any. For instance, the
  ``semver`` specification for version numbers  defines a version as composed
  primarily of three dot-separated numeric segments named major, minor and patch.



Normalized, canonical representation and validation
-----------------------------------------------------

The construction and validation rules are designed such that a ``vers`` is
easier to read and understand by human and straight forward to process by tools,
attempting to avoid the creation of empty or impossible version ranges.

- Spaces are not significant and removed in a canonical form. For example
  "<1.2.3|>=2.0" and " <  1.2. 3 | > = 2  . 0" are equivalent.

- A version range specifier contains only printable ASCII letters, digits and
  punctuation.

- The URI scheme and versioning scheme are always lowercase as in ``vers:npm``. 

- The versions are case-sensitive, and a versioning scheme may specify its own
  case sensitivity.

- If a ``version`` in a ``<version-constraint>`` contains separator or
  comparator characters (i.e. ``><=!*|``), it must be quoted using the URL
  quoting rules. This should be rare in practice.

The list of ``<version-constraint>s`` of a range are signposts in the version
timeline of a package. With these few and simple validation rules, we can avoid
the creation of most empty or impossible version ranges:

- **Constraints are sorted by version**. The canonical ordering is the versions
  order. The ordering of ``<version-constraint>`` is not significant otherwise
  but this sort order is needed when check if a version is contained in a range.

- **Versions are unique**. Each ``version`` must be unique in a range and can
  occur only once in any ``<version-constraint>`` of a range specifier,
  irrespective of its comparators. Tools must report an error for duplicated
  versions.

- **There is only one star**: "*" must only occur once and alone in a range,
  without any other constraint or version.

Starting from a de-duplicated and sorted list of constraints, these extra rules
apply to the comparators of any two contiguous constraints to be valid:

- "!=" constraint can be followed by a constraint using any comparator, i.e.,
  any of "=", "!=", ">", ">=", "<", "<=" as comparator (or no constraint).

Ignoring all constraints with "!=" comparators:

- A "=" constraint must be followed only by a constraint with one of "=", ">",
  ">=" as comparator (or no constraint).

And ignoring all constraints with "=" or "!=" comparators, the sequence of
constraint comparators must be an alternation of greater and lesser comparators:

- "<" and "<=" must be followed by one of ">", ">=" (or no constraint).
- ">" and ">=" must be followed by one of "<", "<=" (or no constraint).

Tools must report an error for such invalid ranges.


Parsing and validating version range specifiers
-------------------------------------------------

To parse a version range specifier string:

- Remove all spaces and tabs.
- Start from left, and split once on colon ":".
- The left hand side is the URI-scheme that must be lowercase.
  - Tools must validate that the URI-scheme value is ``vers``.
- The right hand side is the specifier.

- Split the specifier from left once on a slash "/".

- The left hand side is the <versioning-scheme> that must be lowercase.
  Tools should validate that the <versioning-scheme> is a known scheme.

- The right hand side is a list of one or more constraints.
  Tools must validate that this constraints string is not empty ignoring spaces.

- If the constraints string is equal to "*", the ``<version-constraint>`` is "*".
  Parsing is done and no further processing is needed for this ``vers``. A tool
  should report an error if there are extra characters beyond "*". 

- Strip leading and trailing pipes "|" from the constraints string.
- Split the constraints on pipe "|". The result is a list of ``<version-constraint>``.
  Consecutive pipes must be treated as one and leading and trailing pipes ignored.

- For each ``<version-constraint>``:
  - Determine if the ``<version-constraint>`` starts with one of the two comparators:

    - If it starts with ">=", then the comparator is ">=".
    - If it starts with "<=", then the comparator is "<=".
    - If it starts with "!=", then the comparator is "!=".
    - If it starts with "<",  then the comparator is "<".
    - If it starts with ">",  then the comparator is ">".

    - Remove the comparator from ``<version-constraint>`` string start. The
      remaining string is the version.

  - Otherwise the version is the full ``<version-constraint>`` string (which implies
    an equality comparator of "=")

  - Tools should validate and report an error if the version is empty.

  - If the version contains a percent "%" character, apply URL quoting rules
    to unquote this string.

  - Append the parsed (comparator, version) to the constraints list.

Finally:

- The results are the ``<versioning-scheme>`` and the list of ``<comparator, version>``
  constraints.

Tools should optionally validate and simplify the list of ``<comparator, version>``
constraints once parsing is complete:

- Sort and validate the list of constraints.
- Simplify the list of constraints.


Version constraints simplification
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Tools can simplify a list of ``<version-constraint>`` using this approach:

These pairs of contiguous constraints with these comparators are valid:

- != followed by anything
- =, <, or <= followed by =, !=, >, or >=
- >, or >= followed by !=, <, or <=

These pairs of contiguous constraints with these comparators are redundant and
invalid (ignoring any != since they can show up anywhere):

- =, < or <= followed by < or <=: this is the same as < or <=
- > or >= followed by =, > or >=: this is the same as > or >=


A procedure to remove redundant constraints can be:

- Start from a list of constraints of comparator and version, sorted by version
  and where each version occurs only once in any constraint.

- If the constraints list contains a single constraint (star, equal or anything)
  return this list and simplification is finished.

- Split the constraints list in two sub lists:

    - a list of "unequal constraints" where the comparator is "!="
    - a remainder list of "constraints" where the comparator is not "!="

- If the remainder list of "constraints" is empty, return the "unequal constraints"
  list and simplification is finished.

- Iterate over the constraints list, considering the current and next contiguous
  constraints, and the previous constraint (e.g., before current) if it exists:

    - If current comparator is ">" or ">=" and next comparator is "=", ">" or ">=",
      discard next constraint

    - If current comparator is "=", "<" or "<="  and next comparator is <" or <=",
      discard current constraint. Previous constraint becomes current if it exists.

    - If there is a previous constraint:

        - If previous comparator is ">" or ">=" and current comparator is "=", ">" or ">=",
          discard current constraint

        - If previous comparator is "=", "<" or "<=" and current comparator is <" or <=",
          discard previous constraint

- Concatenate the "unequal constraints" list and the filtered "constraints" list
- Sort by version and return.


Checking if a version is contained within a range
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

To check if a "tested version" is contained within a version range:

- Start from a parsed a version range specifier with:

  - a versioning scheme
  - a list of constraints of comparator and version, sorted by version
    and where each version occurs only once in any constraint.

- If the constraint list contains only one item and the comparator is "*",
  then the "tested version" is IN the range. Check is finished.

- Select the version equality and comparison procedures suitable for this
  versioning scheme and use these for all version comparisons performed below.

- If the "tested version" is equal to the any of the constraint version
  where the constraint comparator is for equality (any of "=", "<=", or ">=")
  then the "tested version" is in the range. Check is finished.

- If the "tested version" is equal to the any of the constraint version where
  the constraint comparator is "=!" then the "tested version" is NOT in the
  range. Check is finished.

- Split the constraint list in two sub lists:

  - a first list where the comparator is "=" or "!="
  - a second list where the comparator is neither "=" nor "!="

- Iterate over the current and next contiguous constraints pairs (aka. pairwise)
  in the second list.

- For each current and next constraint:

    - If this is the first iteration and current comparator is "<" or <="
      and the "tested version" is less than the current version
      then the "tested version" is IN the range. Check is finished.

    - If this is the last iteration and next comparator is ">" or >="
      and the "tested version" is greater than the next version
      then the "tested version" is IN the range. Check is finished.

    - If current comparator is ">" or >=" and next comparator is "<" or <="
      and the "tested version" is greater than the current version
      and the "tested version" is less than the next version
      then the "tested version" is IN the range. Check is finished.

    - If current comparator is "<" or <=" and next comparator is ">" or >="
      then these versions are out the range. Continue to the next iteration.

- Reaching here without having finished the check before means that the
  "tested version" is NOT in the range.


Notes and caveats
~~~~~~~~~~~~~~~~~~~

- Comparing versions from two different versioning schemes is an error. Even
  though there may be some similarities between the ``semver`` version of an npm
  and the ``deb`` version of its Debian packaging, the way versions are compared
  specific to each versioning scheme and may be different. Tools should report
  an error in this case.

- All references to sorting or ordering of version constraints means sorting
  by version. And sorting by versions always implies using the versioning
  scheme-specified version comparison and ordering.
