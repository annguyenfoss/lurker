/* LGPL-3.0-or-later. Public libc ABI, generated from native FreeBSD headers. */
package org.cryptomator.jfuse.freebsd.amd64;

import org.cryptomator.jfuse.freebsd.amd64.extr.fuse3.statfs;
import org.cryptomator.jfuse.freebsd.amd64.extr.mount.mount_h;
import java.io.IOException;
import java.lang.foreign.*;
import java.lang.invoke.MethodHandle;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

final class FreeBsdMounts {
    record Entry(int fsid0, int fsid1, long flags, int owner,
                 String type, String source, String target) {
        boolean sameFsid(Entry other) { return fsid0==other.fsid0 && fsid1==other.fsid1; }
    }
    private static final MethodHandle GETFSSTAT = Linker.nativeLinker().downcallHandle(
        Linker.nativeLinker().defaultLookup().find("getfsstat").orElseThrow(),
        FunctionDescriptor.of(ValueLayout.JAVA_INT,ValueLayout.ADDRESS,
                              ValueLayout.JAVA_LONG,ValueLayout.JAVA_INT));
    private static int query(MemorySegment memory, long bytes) throws IOException {
        try { return (int)GETFSSTAT.invokeExact(memory,bytes,mount_h.MNT_NOWAIT()); }
        catch (Throwable error) { throw new IOException("Native NOWAIT mount snapshot failed",error); }
    }
    private static String string(MemorySegment bytes) throws IOException {
        byte[] value=bytes.toArray(ValueLayout.JAVA_BYTE);
        int end=0;
        while (end<value.length && value[end]!=0) end++;
        if (end==value.length) throw new IOException("Unterminated native mount identity");
        try { return StandardCharsets.UTF_8.newDecoder().decode(ByteBuffer.wrap(value,0,end)).toString(); }
        catch (CharacterCodingException error) { throw new IOException("Unrepresentable native mount identity",error); }
    }
    /** NOWAIT avoids the known blocking MNT_WAIT enumeration. A skipped busy
     * mount is never sufficient close authority: an owned native normal-unmount
     * receipt and natural loop termination are independently required by the
     * lease. Full buffers are never absence. */
    static List<Entry> snapshot() throws IOException {
        FreeBsdNative.requireLoginUser();
        for (int attempt=0;attempt<4;attempt++) {
            int count=query(MemorySegment.NULL,0L);
            if (count<0 || count>8192) throw new IOException("Invalid native mount count");
            int capacity=Math.addExact(count,32*(attempt+1));
            try (var arena=Arena.ofConfined()) {
                long bytes=Math.multiplyExact(statfs.sizeof(),capacity);
                var memory=arena.allocate(bytes,statfs.alignof());
                int actual=query(memory,bytes);
                if (actual<0) throw new IOException("Native mount snapshot refused");
                if (actual>=capacity) continue;
                var entries=new ArrayList<Entry>(actual);
                for (int i=0;i<actual;i++) {
                    var row=statfs.asSlice(memory,i);
                    entries.add(new Entry(statfs.fsid0(row),statfs.fsid1(row),statfs.f_flags(row),
                        statfs.f_owner(row),string(statfs.f_fstypename(row)),
                        string(statfs.f_mntfromname(row)),string(statfs.f_mntonname(row))));
                }
                return List.copyOf(entries);
            }
        }
        throw new IOException("Native mount table kept changing; ownership unknown");
    }
    static Entry capture(List<Entry> before,List<Entry> after,String source,String target,int uid) throws IOException {
        var matches=after.stream().filter(e->e.target.equals(target)).toList();
        if (matches.isEmpty()) return null; // helper publication may follow INIT
        if (matches.size()!=1) throw new IOException("Ambiguous native mount target");
        var found=matches.getFirst();
        if (!found.type.equals("fusefs") || !found.source.equals(source) || found.owner!=uid
                || (found.fsid0==0 && found.fsid1==0)
                || (found.flags & mount_h.MNT_RDONLY())!=0
                || before.stream().anyMatch(e->found.sameFsid(e) || e.source.equals(source))
                || after.stream().filter(found::sameFsid).count()!=1
                || after.stream().filter(e->e.source.equals(source)).count()!=1) {
            throw new IOException("Native mounted identity differs; retaining channel");
        }
        return found;
    }
    static boolean absent(Entry owned,List<Entry> current) throws IOException {
        boolean present=false;
        for (var row:current) {
            if (row.sameFsid(owned)) {
                if (!row.equals(owned)) throw new IOException("Owned mount moved or changed; retaining channel");
                if (present) throw new IOException("Duplicate owned mount identity; retaining channel");
                present=true;
            } else if (row.source.equals(owned.source) || row.target.equals(owned.target)) {
                throw new IOException("Mount source or target replaced; retaining channel");
            }
        }
        return !present;
    }
}
