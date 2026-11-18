# OpenGVA - the crew station's image, as a bootable container (bootc) on
# Rocky Linux.
#
#   make container  build the image: localhost/opengva:hmi
#   make test       run the HMI's Rust tests on the host
#   make ui         serve the UI in a browser (no Rust, no image)
#
# Built by podman in layers, each a Containerfile on the one before:
#
#   os/base         Rocky, its kernel and systemd, and the boot: what every image has
#   os/hmi          the crew station's own part, with the HMI on it
#                   and opengva-rpm, which every part compiled or added is made an RPM by
#
# Whatever is to be on the image as well goes in layers on that; last, if it
# is to be hardened, os/hardened (nobody becomes root by asking). A disk is
# made of the image by osbuild's image-builder (build raw --bootc-ref), or a
# machine already running one moves to it by bootc switch.

TOP   := $(patsubst %/,%,$(dir $(abspath $(lastword $(MAKEFILE_LIST)))))

# Which Rocky every image is: os/base is built on it, and what os/hmi compiles.
ROCKY  := git.resf.org/sig_containers/rocky-bootc-minimal:10.2
PODMAN ?= podman
# The RPMs': VERSION by hand, RELEASE the commit they are built from.
VERSION ?= 0.1
RELEASE ?= $(shell git -C $(TOP) rev-list --count HEAD).g$(shell git -C $(TOP) rev-parse --short HEAD)

.PHONY: container ui test clean

# Podman keeps what has not changed, so a build after a change to the HMI
# compiles the HMI and no more.
container:
	$(PODMAN) build -t localhost/opengva:base --build-arg ROCKY=$(ROCKY) $(TOP)/os/base
	$(PODMAN) build -t localhost/opengva:hmi --build-arg BASE=localhost/opengva:base \
		--build-arg ROCKY=$(ROCKY) --build-arg VERSION=$(VERSION) --build-arg RELEASE=$(RELEASE) \
		-f $(TOP)/os/hmi/Containerfile $(TOP)

ui:
	cd $(TOP)/app && npm install && npm run dev

# These crates have no windowing dependencies, so this needs only Rust.
test:
	cd $(TOP)/app && cargo test -p hmi-core -p hmi-entry -p hmi-access -p bezel-input -p hmi-feed -p us-keyboard
	cd $(TOP)/app/viewers/vnc && cargo test -p vnc-view
	cd $(TOP)/app/viewers/web && cargo test -p web-view

# What podman has kept to build it again quickly stays: "podman image prune".
clean:
	-$(PODMAN) rmi localhost/opengva:hmi
