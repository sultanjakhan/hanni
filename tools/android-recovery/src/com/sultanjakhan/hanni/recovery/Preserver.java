package com.sultanjakhan.hanni.recovery;

import java.io.*;
import java.nio.channels.Channels;
import java.nio.file.*;
import java.nio.file.attribute.BasicFileAttributes;
import java.nio.charset.StandardCharsets;
import java.security.*;
import java.security.interfaces.RSAPublicKey;
import java.security.spec.MGF1ParameterSpec;
import java.util.*;
import java.util.zip.*;
import javax.crypto.*;
import javax.crypto.spec.*;

/** Raw filesystem bytes only. No SQLite, app initialization, repair, writes or deletion. */
public final class Preserver {
  public static final long CIPHER_CAP = 512L * 1024 * 1024, PLAIN_CAP = 2L * 1024 * 1024 * 1024;
  private static final int ENTRY_CAP = 50000;
  public static final class Result { public final int files; public final boolean complete;
    Result(int files, boolean complete) { this.files = files; this.complete = complete; } }
  private final Path root;
  private final List<Map<String,Object>> copied = new ArrayList<>(), skipped = new ArrayList<>(), errors = new ArrayList<>();
  private long total;
  private int entries;
  private Preserver(Path root) { this.root = root; }

  public static Result export(Path root, OutputStream output, PublicKey recipient) throws Exception {
    if (!(recipient instanceof RSAPublicKey)) throw new IOException("recipient_rejected");
    int bits = ((RSAPublicKey)recipient).getModulus().bitLength();
    if (bits != 3072 && bits != 4096) throw new IOException("recipient_rejected");
    root = root.toRealPath();
    if (!Files.isDirectory(root, LinkOption.NOFOLLOW_LINKS)) throw new IOException("root_rejected");
    Preserver p = new Preserver(root);
    byte[] key = new byte[32], nonce = new byte[12]; SecureRandom random = new SecureRandom(); random.nextBytes(key); random.nextBytes(nonce);
    Cipher rsa = Cipher.getInstance("RSA/ECB/OAEPWithSHA-256AndMGF1Padding");
    rsa.init(Cipher.ENCRYPT_MODE, recipient, new OAEPParameterSpec("SHA-256", "MGF1", MGF1ParameterSpec.SHA256, PSource.PSpecified.DEFAULT));
    byte[] wrapped = rsa.doFinal(key);
    ByteArrayOutputStream bytes = new ByteArrayOutputStream(); DataOutputStream header = new DataOutputStream(bytes);
    header.write("HANNIREC1".getBytes(StandardCharsets.US_ASCII)); header.writeInt(wrapped.length); header.write(wrapped); header.write(nonce); header.flush();
    Cipher aes = Cipher.getInstance("AES/GCM/NoPadding");
    try { aes.init(Cipher.ENCRYPT_MODE, new SecretKeySpec(key, "AES"), new GCMParameterSpec(128, nonce)); }
    finally { Arrays.fill(key, (byte)0); }
    byte[] aad = bytes.toByteArray(); aes.updateAAD(aad);
    OutputStream bounded = new FilterOutputStream(output) {
      long count;
      @Override public void write(int value) throws IOException { if (++count > CIPHER_CAP) throw new IOException("cipher_cap"); out.write(value); }
      @Override public void write(byte[] b, int off, int len) throws IOException { if (len > CIPHER_CAP - count) throw new IOException("cipher_cap"); out.write(b, off, len); count += len; }
      @Override public void close() throws IOException { flush(); } // socket stays open for shutdownOutput/ACK
    };
    bounded.write(aad);
    try (ZipOutputStream zip = new ZipOutputStream(new CipherOutputStream(bounded, aes))) {
      zip.setLevel(Deflater.BEST_SPEED);
      try { p.walk(root, zip); } catch (IOException failure) { p.errors.add(item("path", "appdata/", "code", "traversal_failed")); }
      List<Map<String,Object>> expected = new ArrayList<>();
      int primaryCopied = 0;
      for (String name : new String[]{"files/hanni.db", "files/hanni.db-wal", "files/hanni.db-shm", "files/hanni.db-journal", "hanni.db", "hanni.db-wal", "hanni.db-shm", "hanni.db-journal"}) {
        String path = "appdata/" + name, status;
        try {
          p.checkParents(root.resolve(name));
          BasicFileAttributes attr = Files.readAttributes(root.resolve(name), BasicFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
          status = attr.isRegularFile() ? "present_regular" : "present_non_regular";
          int matches = 0;
          for (Map<String,Object> row : p.copied) if (path.equals(row.get("path")) && Boolean.TRUE.equals(row.get("complete"))) matches++;
          if (!attr.isRegularFile() || matches != 1) p.errors.add(item("path", path, "code", "primary_not_preserved"));
          else if (name.equals("hanni.db") || name.equals("files/hanni.db")) primaryCopied++;
        } catch (NoSuchFileException absent) {
          status = "missing";
        } catch (IOException inaccessible) {
          status = "unknown"; p.errors.add(item("path", path, "code", "primary_stat_failed"));
        }
        expected.add(item("path", path, "status", status));
      }
      if (primaryCopied == 0) p.errors.add(item("path", "appdata/", "code", "primary_missing_or_incomplete"));
      Map<String,Object> inventory = item("schema", "hanni.forensic-preservation.v1", "complete", p.errors.isEmpty(), "copied", p.copied,
        "skipped", p.skipped, "errors", p.errors, "expected_primary", expected, "files", p.copied.size(), "bytes", p.total);
      zip.putNextEntry(new ZipEntry("recovery/inventory.json")); zip.write(json(inventory).getBytes(StandardCharsets.UTF_8)); zip.closeEntry();
    }
    output.flush(); return new Result(p.copied.size(), p.errors.isEmpty());
  }

  private void walk(Path directory, ZipOutputStream zip) throws IOException {
    checkParents(directory);
    List<Path> children = new ArrayList<>();
    try (DirectoryStream<Path> stream = Files.newDirectoryStream(directory)) { for (Path child : stream) { if (++entries > ENTRY_CAP) throw new IOException("entry_cap"); children.add(child); } }
    children.sort(Comparator.comparing(p -> p.getFileName().toString()));
    for (Path child : children) {
      String rel = root.relativize(child).toString().replace(File.separatorChar, '/'); String entry = "appdata/" + rel;
      if (directory.equals(root) && (rel.equals("cache") || rel.equals("code_cache"))) { skipped.add(item("path", entry, "code", "cache_excluded")); continue; }
      if (rel.indexOf('\\') >= 0 || rel.indexOf(':') >= 0 || rel.startsWith("/") || Arrays.asList(rel.split("/")).contains("..")) { errors.add(item("path", entry, "code", "unsafe_name")); continue; }
      BasicFileAttributes attr;
      try { attr = Files.readAttributes(child, BasicFileAttributes.class, LinkOption.NOFOLLOW_LINKS); }
      catch (IOException error) { errors.add(item("path", entry, "code", "stat_failed")); continue; }
      if (attr.isSymbolicLink()) { skipped.add(item("path", entry, "code", "symlink_excluded")); continue; }
      if (attr.isDirectory()) { try { walk(child, zip); } catch (IOException error) { errors.add(item("path", entry, "code", "directory_failed")); } }
      else if (attr.isRegularFile()) copy(child, entry, attr, zip);
      else skipped.add(item("path", entry, "code", "non_regular"));
    }
  }

  private void checkParents(Path path) throws IOException {
    if (!path.normalize().startsWith(root)) throw new IOException("path_rejected");
    Path cursor = root;
    if (!Files.isDirectory(root, LinkOption.NOFOLLOW_LINKS)) throw new IOException("root_changed");
    for (Path part : root.relativize(path)) {
      cursor = cursor.resolve(part);
      if (Files.isSymbolicLink(cursor)) throw new IOException("path_rejected");
    }
  }

  private void copy(Path source, String entry, BasicFileAttributes before, ZipOutputStream zip) throws IOException {
    if (before.size() < 0 || before.size() > PLAIN_CAP - total) { errors.add(item("path", entry, "code", "plain_cap")); return; }
    InputStream input;
    try { checkParents(source); input = Channels.newInputStream(Files.newByteChannel(source, new HashSet<OpenOption>(Arrays.asList(StandardOpenOption.READ, LinkOption.NOFOLLOW_LINKS)))); }
    catch (IOException error) { errors.add(item("path", entry, "code", "open_failed")); return; }
    MessageDigest hash;
    try { hash = MessageDigest.getInstance("SHA-256"); } catch (GeneralSecurityException impossible) { throw new IOException("crypto_failed"); }
    long count = 0; boolean failed = false;
    try (InputStream in = input) {
      zip.putNextEntry(new ZipEntry(entry));
      byte[] buffer = new byte[65536];
      while (true) {
        int n;
        try { n = in.read(buffer); } catch (IOException error) { failed = true; errors.add(item("path", entry, "code", "read_failed")); break; }
        if (n < 0) break;
        if (n > PLAIN_CAP - total) { failed = true; errors.add(item("path", entry, "code", "plain_cap")); break; }
        zip.write(buffer, 0, n); hash.update(buffer, 0, n); count += n; total += n;
      }
      Arrays.fill(buffer, (byte)0);
    }
    zip.closeEntry();
    long afterTime = -1;
    try {
      checkParents(source); BasicFileAttributes after = Files.readAttributes(source, BasicFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
      afterTime = after.lastModifiedTime().toMillis() / 1000;
      if (!after.isRegularFile() || count != before.size() || after.size() != before.size() || !after.lastModifiedTime().equals(before.lastModifiedTime()) || !Objects.equals(after.fileKey(), before.fileKey())) { failed = true; errors.add(item("path", entry, "code", "changed_during_copy")); }
    } catch (IOException error) { failed = true; errors.add(item("path", entry, "code", "post_stat_failed")); }
    copied.add(item("path", entry, "bytes", count, "sha256", hex(hash.digest()), "size_before", before.size(), "mtime_before_sec", before.lastModifiedTime().toMillis() / 1000, "mtime_after_sec", afterTime, "complete", !failed));
  }

  private static String hex(byte[] bytes) { StringBuilder out = new StringBuilder(); for (byte b : bytes) out.append(String.format(Locale.ROOT, "%02x", b & 255)); return out.toString(); }
  private static Map<String,Object> item(Object... values) { Map<String,Object> result = new LinkedHashMap<>(); for (int i=0; i<values.length; i+=2) result.put((String)values[i], values[i+1]); return result; }
  private static String json(Object value) {
    if (value == null) return "null";
    if (value instanceof Number || value instanceof Boolean) return value.toString();
    if (value instanceof Map) { StringJoiner join = new StringJoiner(",", "{", "}"); for (Map.Entry<?,?> e : ((Map<?,?>)value).entrySet()) join.add(json(e.getKey()) + ":" + json(e.getValue())); return join.toString(); }
    if (value instanceof List) { StringJoiner join = new StringJoiner(",", "[", "]"); for (Object item : (List<?>)value) join.add(json(item)); return join.toString(); }
    StringBuilder out = new StringBuilder("\"");
    for (char c : value.toString().toCharArray()) { if (c == '"' || c == '\\') out.append('\\').append(c); else if (c < 32) out.append(String.format(Locale.ROOT, "\\u%04x", (int)c)); else out.append(c); }
    return out.append('"').toString();
  }
}
