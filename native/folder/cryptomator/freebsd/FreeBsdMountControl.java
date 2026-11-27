/* LGPL-3.0-or-later. The provider must not bypass this close boundary. */
package org.cryptomator.jfuse.freebsd.amd64;

import java.io.IOException;
import java.util.concurrent.TimeoutException;

public interface FreeBsdMountControl extends java.util.function.BooleanSupplier {
    /** Normal native unmount, bound identity absence, natural loop completion. */
    void closeNormally() throws IOException, TimeoutException;
}
