################################################################################
#
# ledbetter
#
################################################################################

LEDBETTER_VERSION = 1.0
LEDBETTER_SITE = $(BR2_EXTERNAL_LEDBETTER_FIRMWARE_PATH)/../../ledbetter-firmware
LEDBETTER_SITE_METHOD = local
LEDBETTER_LICENSE = GPL-3.0+
LEDBETTER_LICENSE_FILES = COPYING

LEDBETTER_DEPENDENCIES = host-rustc host-pkgconf openssl

LEDBETTER_CARGO_ENV = \
	CARGO_HOME=$(HOST_DIR)/share/cargo \
	OPENSSL_DIR=$(STAGING_DIR)/usr \
	TARGET_CC=$(TARGET_CC) \
	BINDGEN_EXTRA_CLANG_ARGS=--sysroot=$(STAGING_DIR)

LEDBETTER_CARGO_MODE = $(if $(BR2_ENABLE_DEBUG),debug,release)

LEDBETTER_BIN_DIR = target/$(RUSTC_TARGET_NAME)/$(LEDBETTER_CARGO_MODE)

LEDBETTER_CARGO_OPTS = \
	$(if $(BR2_ENABLE_DEBUG),,--release) \
	--target=$(RUSTC_TARGET_NAME) \
	--manifest-path=$(@D)/Cargo.toml \
  --no-default-features \
  --features rpi

define LEDBETTER_BUILD_CMDS
	$(TARGET_MAKE_ENV) $(LEDBETTER_CARGO_ENV) \
		cargo build $(LEDBETTER_CARGO_OPTS)
endef

define LEDBETTER_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/$(LEDBETTER_BIN_DIR)/ledbetter-client \
		$(TARGET_DIR)/usr/bin/ledbetter-client
endef

define LEDBETTER_INSTALL_INIT_SYSV
	$(INSTALL) -D -m 0755 $(LEDBETTER_PKGDIR)S99ledbetter \
		$(TARGET_DIR)/etc/init.d/S99ledbetter
endef

$(eval $(generic-package))
