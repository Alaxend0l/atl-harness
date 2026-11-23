// SPDX-License-Identifier: AGPL-3.0-or-later
// Rig helper: libcore's FileDescriptor/Socket internals (getInt$/setInt$/
// getFileDescriptor$) do not exist in stock JVM java.*; the rig's patched
// tree routes those calls here instead (reflection into the stock `fd`
// fields). This file is copied into the rig's build tree only — never into
// the fork.
package android.atl.rig;

import java.io.FileDescriptor;
import java.lang.reflect.Field;

public class RigFd {
  private static Field f(String cn, String fn) {
    try {
      Class<?> c = Class.forName(cn);
      Field f = c.getDeclaredField(fn);
      f.setAccessible(true);
      return f;
    } catch (Exception e) {
      throw new RuntimeException(e);
    }
  }

  public static int getInt(FileDescriptor fd) {
    try {
      return f("java.io.FileDescriptor", "fd").getInt(fd);
    } catch (Exception e) {
      return -1;
    }
  }

  public static void setInt(FileDescriptor fd, int v) {
    try {
      f("java.io.FileDescriptor", "fd").setInt(fd, v);
    } catch (Exception ignored) {
    }
  }

  public static FileDescriptor socketFd(java.net.Socket s) {
    try {
      return (FileDescriptor) f("java.net.Socket", "fd").get(s);
    } catch (Exception e) {
      return null;
    }
  }

  public static FileDescriptor dgramFd(java.net.DatagramSocket s) {
    try {
      return (FileDescriptor) f("java.net.DatagramSocket", "fd").get(s);
    } catch (Exception e) {
      return null;
    }
  }
}
