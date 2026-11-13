module org.lurker.cryptomator {
    requires org.cryptomator.cryptofs;
    requires org.cryptomator.cryptolib;
    requires org.cryptomator.integrations.api;
    requires org.cryptomator.frontend.fuse;
    uses org.cryptomator.integrations.mount.MountService;
}
