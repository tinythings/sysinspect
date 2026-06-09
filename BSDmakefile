.if exists(/usr/local/bin/gmake)
GMAKE_CMD=/usr/local/bin/gmake
.elif exists(/usr/bin/gmake)
GMAKE_CMD=/usr/bin/gmake
.else
.error GNU make not found. Install 'gmake' and run 'gmake <target>'.
.endif

.if empty(.TARGETS)
TARGETS=help
.else
TARGETS=${.TARGETS}
.endif

all:
	@${GMAKE_CMD} -f Makefile ${TARGETS}

.for tgt in ${.TARGETS}
${tgt}: all
.endfor
