	/* FreeBSD owns this lifecycle rather than the base class's finally-close
	 * arena and unbounded path-probe behavior. Strong references preserve failed
	 * startup handles until an explicit successful close, not merely until GC. */
	private static final java.util.Set<FuseImpl> RETAINED = java.util.concurrent.ConcurrentHashMap.newKeySet();
	private final java.util.concurrent.CountDownLatch nativeInit = new java.util.concurrent.CountDownLatch(1);
	private final java.util.concurrent.ExecutorService nativeExecutor = java.util.concurrent.Executors.newSingleThreadExecutor(
		Thread.ofPlatform().name("jfuse-freebsd-loop").daemon(false).factory());
	private FreeBsdMountLease nativeLease;
	private boolean mountUsed;

	@Override
	public synchronized void mount(String progName, java.nio.file.Path point, String... flags) throws FuseMountFailedException {
		if (mountUsed || !fuseArena.scope().isAlive()) throw new IllegalStateException("Native session cannot be reused");
		if (!progName.matches("lurker-cryptomator-[a-f0-9]{32}")) throw new IllegalArgumentException("Nonce-bound native fsname required");
		for (String flag:flags) if (flag.contains("fsname") || flag.contains("subtype"))
			throw new IllegalArgumentException("Native source identity is provider-controlled");
		java.nio.file.Path canonical;
		try {
			canonical=point.toRealPath();
			if (!java.nio.file.Files.isDirectory(canonical,java.nio.file.LinkOption.NOFOLLOW_LINKS)
					|| (int)java.nio.file.Files.getAttribute(canonical,"unix:uid")!=FreeBsdNative.uid()
					|| ((int)java.nio.file.Files.getAttribute(canonical,"unix:mode") & 0022)!=0)
				throw new java.io.IOException("Private login-user mount directory required");
		} catch (java.io.IOException error) { throw new FuseMountFailedException(error.getMessage()); }
		mountUsed=true;
		var args=new java.util.ArrayList<String>();
		args.add(progName);args.addAll(java.util.List.of(flags));
		args.add("-ofsname="+progName);args.add("-f");args.add(canonical.toString());
		try {
			var lease=(FreeBsdMountLease)this.mount(args);
			var loop=nativeExecutor.submit(lease::loop);
			long until=System.nanoTime()+java.util.concurrent.TimeUnit.SECONDS.toNanos(10);
			while (!nativeInit.await(10,java.util.concurrent.TimeUnit.MILLISECONDS)) {
				if (loop.isDone() || System.nanoTime()>=until)
					throw new FuseMountFailedException("Native INIT unproven; retaining channel and callbacks");
			}
			// The ordinary loop serves INIT before the asynchronous mount helper's
			// publication is required. No Files.getFileStore/getattr global probe.
			lease.captureAfterInit();
			if (loop.isDone()) throw new FuseMountFailedException("Native loop ended during publication; retaining lease");
		} catch (InterruptedException error) {
			Thread.currentThread().interrupt();throw new FuseMountFailedException("Interrupted; retaining native startup lease");
		} catch (java.io.IOException | java.util.concurrent.TimeoutException error) {
			throw new FuseMountFailedException(error.getMessage());
		}
	}

	@Override
	protected FuseMount mount(List<String> args) throws FuseMountFailedException {
		var fuseArgs=parseArgs(args);
		var target=fuseArgs.mountPoint().getString(0);
		java.util.List<FreeBsdMounts.Entry> before;
		try {
			before=FreeBsdMounts.snapshot();
			if (before.stream().anyMatch(e->e.target().equals(target)))
				throw new java.io.IOException("Native target is already mounted");
		} catch (java.io.IOException error) { throw new FuseMountFailedException(error.getMessage()); }
		var fuse=createFuseFS(fuseArgs);
		nativeLease=new FreeBsdMountLease(fuse,before,args.getFirst(),target);
		RETAINED.add(this);
		nativeLease.attemptingMount();
		if (fuse_h.fuse_mount(fuse,fuseArgs.mountPoint())!=0)
			throw new FuseMountFailedException("Native mount failed; publication remains unproven and lease retained");
		return nativeLease;
	}

	@Override
	public synchronized boolean getAsBoolean() {
		return fuseArena.scope().isAlive() && nativeLease!=null && nativeLease.retryableBusy();
	}

	@Override
	public synchronized void closeNormally() throws java.io.IOException,java.util.concurrent.TimeoutException {
		if (!fuseArena.scope().isAlive()) return;
		if (nativeLease!=null) nativeLease.closeNormally();
		close();
	}

	@Override
	public synchronized void close() throws java.util.concurrent.TimeoutException {
		if (!fuseArena.scope().isAlive()) return;
		try {
			if (nativeLease!=null) nativeLease.dispose();
		} catch (java.io.IOException error) {
			var retained=new java.util.concurrent.TimeoutException(error.getMessage());retained.initCause(error);throw retained;
		}
		nativeExecutor.shutdown();
		try {
			if (!nativeExecutor.awaitTermination(1,java.util.concurrent.TimeUnit.SECONDS))
				throw new java.util.concurrent.TimeoutException("Loop executor still live; arena retained");
		} catch (InterruptedException error) {
			Thread.currentThread().interrupt();throw new java.util.concurrent.TimeoutException("Interrupted; arena retained");
		}
		// No finally block: failed checks leave callback memory and state alive.
		fuseArena.close();
		RETAINED.remove(this);
	}
