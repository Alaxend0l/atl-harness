/* SPDX-License-Identifier: AGPL-3.0-or-later
 * JVM rig JNI shims: a handful of libcore natives that ART provides but a
 * stock HotSpot JVM cannot. The rig loads the equivalent of this library so
 * pure-logic framework code paths that touch them run headlessly. Extend by
 * adding Java_<mangled-name> functions; the same extern "C" symbol pattern
 * a future Rust core would use (docs/research/rust-rewrite.md §10).
 */
#include <jni.h>
#include <stdio.h>
#include <string.h>

static const char *name_of(JNIEnv *env, jobject c) {
  static char buf[256];
  jclass cls = (*env)->GetObjectClass(env, c);
  jmethodID getName = (*env)->GetMethodID(env, cls, "getName", "()Ljava/lang/String;");
  jstring s = (jstring)(*env)->CallObjectMethod(env, c, getName);
  const char *utf = (*env)->GetStringUTFChars(env, s, 0);
  strncpy(buf, utf, 255);
  buf[255] = 0;
  (*env)->ReleaseStringUTFChars(env, s, utf);
  return buf;
}

/* libcore ArrayUtils routes unpadded allocations through this ART native.
 * Note: at runtime HotSpot may receive the component type (e.g. "int"),
 * ART receives the array type ("[I") — accept both. */
JNIEXPORT jobject JNICALL Java_dalvik_system_VMRuntime_newUnpaddedArray(JNIEnv *env, jobject thiz, jobject clazz, jint n) {
  const char *cn = name_of(env, clazz);
  if (!strcmp(cn, "[C") || !strcmp(cn, "char")) return (*env)->NewCharArray(env, n);
  if (!strcmp(cn, "[B") || !strcmp(cn, "byte")) return (*env)->NewByteArray(env, n);
  if (!strcmp(cn, "[I") || !strcmp(cn, "int")) return (*env)->NewIntArray(env, n);
  if (!strcmp(cn, "[J") || !strcmp(cn, "long")) return (*env)->NewLongArray(env, n);
  return (*env)->NewObjectArray(env, n, (*env)->FindClass(env, "java/lang/Object"), 0);
}
