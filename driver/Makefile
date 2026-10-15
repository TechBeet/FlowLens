// FlowLens Driver Makefile
!INCLUDE $(WINDDK)/makefile.default

TARGETNAME=flowlens
TARGETPATH=obj
TARGETTYPE=DRIVER

UMDFVERSION=1.0
UMDFLIBS=usbd.lib

INCLUDES=$(INCLUDES);.\include

SOURCE_LIBS=$(SOURCE_LIBS);ntstrsafe.lib;wdmsec.lib

# WFP driver flags
CFLAGS=$(CFLAGS) /W4 /GS /Zi /Fa

# Driver object
SRCS=src\callout.c

# Sources
SOURCES=src\callout.c

!IF "$(TARGETPLATFORM)" == "AMD64"
!ELSEIF "$(TARGETPLATFORM)" == "ARM64"
!ELSE
!ERROR Unsupported target platform
!ENDIF