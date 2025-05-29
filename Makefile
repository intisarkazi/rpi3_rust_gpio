KDIR := ~/dlinux/linux

ARCH := arm64
CROSS_COMPILE := aarch64-linux-gnu-
LLVM := 1


default:
	$(MAKE) -C $(KDIR) M=$(PWD) ARCH=$(ARCH) CROSS_COMPILE=$(CROSS_COMPILE) LLVM=$(LLVM)
	@$(MAKE) cleanup

modules_install: default
	$(MAKE) -C $(KDIR) M=$(PWD) modules_install

clean:
	$(MAKE) -C $(KDIR) M=$(PWD) clean
	@rm -f *.ko *.mod.c *.o *.mod *.mod.o *.mod.cmd *.symvers *.order .*.cmd .*.o.cmd .*.ko.cmd
	@rm -rf .tmp_versions

cleanup:
	@find . -maxdepth 1 ! -name '*.ko' ! -name '*.rs' ! -name 'Makefile' ! -name 'Kbuild' -type f -delete
	@rm -rf .tmp_versions