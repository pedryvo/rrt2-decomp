// Dump every function of the program to JSON (see docs/re-workflow.md).
// Args: <out.json> [symbols.toml]
// @category RRT2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import ghidra.program.model.mem.MemoryBlock;
import java.io.*;
import java.nio.file.*;
import java.util.*;
import java.util.regex.*;

public class dump_program extends GhidraScript {
    static String esc(String s) {
        StringBuilder b = new StringBuilder("\"");
        for (char c : s.toCharArray()) {
            switch (c) {
                case '"': b.append("\\\""); break;
                case '\\': b.append("\\\\"); break;
                case '\n': b.append("\\n"); break;
                case '\r': b.append("\\r"); break;
                case '\t': b.append("\\t"); break;
                default:
                    if (c < 0x20 || c > 0x7e) b.append(String.format("\\u%04x", (int) c));
                    else b.append(c);
            }
        }
        return b.append('"').toString();
    }
    static String hx(Address a) { return "0x" + Long.toHexString(a.getOffset()); }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        String out = args[0];
        Map<Long, String> syms = new HashMap<>();
        if (args.length > 1 && new File(args[1]).exists()) {
            Pattern pa = Pattern.compile("^addr\\s*=\\s*\"0x([0-9a-fA-F]+)\"");
            Pattern pn = Pattern.compile("^name\\s*=\\s*\"([^\"]+)\"");
            Pattern pk = Pattern.compile("^kind\\s*=\\s*\"([^\"]+)\"");
            Long addr = null; String name = null, kind = null;
            List<String> lines = new ArrayList<>(Files.readAllLines(Paths.get(args[1])));
            lines.add("[[symbol]]");
            for (String l : lines) {
                if (l.startsWith("[[symbol]]")) {
                    if (addr != null && name != null && (kind == null || kind.equals("function"))) syms.put(addr, name);
                    addr = null; name = null; kind = null;
                    continue;
                }
                Matcher m;
                if ((m = pa.matcher(l)).find()) addr = Long.parseUnsignedLong(m.group(1), 16);
                else if ((m = pn.matcher(l)).find()) name = m.group(1);
                else if ((m = pk.matcher(l)).find()) kind = m.group(1);
            }
        }
        // Remember Function ID matches before we rename anything.
        Set<Long> fidAddrs = new HashSet<>();
        Map<Long, String> fidNames = new HashMap<>();
        BookmarkManager bm = currentProgram.getBookmarkManager();
        Iterator<Bookmark> bit = bm.getBookmarksIterator();
        while (bit.hasNext()) {
            Bookmark b = bit.next();
            if (b.getCategory() != null && b.getCategory().toLowerCase().contains("function id")) {
                fidAddrs.add(b.getAddress().getOffset());
                fidNames.put(b.getAddress().getOffset(), b.getComment());
            }
        }
        FunctionManager fm = currentProgram.getFunctionManager();
        int applied = 0;
        for (Map.Entry<Long, String> e : syms.entrySet()) {
            Function f = fm.getFunctionAt(toAddr(e.getKey()));
            if (f != null) {
                try { f.setName(e.getValue(), SourceType.USER_DEFINED); applied++; } catch (Exception ex) { printerr("rename " + e.getValue() + ": " + ex); }
            }
        }
        println("applied " + applied + " symbol names");

        ReferenceManager rm = currentProgram.getReferenceManager();
        Listing listing = currentProgram.getListing();
        StringBuilder sb = new StringBuilder();
        sb.append("{\n\"program\":").append(esc(currentProgram.getName()))
          .append(",\n\"image_base\":").append(esc(hx(currentProgram.getImageBase())))
          .append(",\n\"functions\":[\n");
        boolean first = true;
        int n = 0;
        for (Function f : fm.getFunctions(true)) {
            if (f.isExternal()) continue;
            AddressSetView body = f.getBody();
            TreeSet<Long> callees = new TreeSet<>(), callers = new TreeSet<>(), data = new TreeSet<>();
            TreeSet<String> apis = new TreeSet<>();
            TreeMap<Long, String> strings = new TreeMap<>();
            for (Reference r : rm.getReferencesTo(f.getEntryPoint())) {
                if (r.getReferenceType().isCall() || r.getReferenceType().isJump()) {
                    Function c = fm.getFunctionContaining(r.getFromAddress());
                    if (c != null && !c.getEntryPoint().equals(f.getEntryPoint())) callers.add(c.getEntryPoint().getOffset());
                }
            }
            for (Address a : body.getAddresses(true)) {
                for (Reference r : rm.getReferencesFrom(a)) {
                    Address to = r.getToAddress();
                    if (to.isExternalAddress()) {
                        Symbol s = currentProgram.getSymbolTable().getPrimarySymbol(to);
                        if (s != null) {
                            ExternalLocation el = currentProgram.getExternalManager().getExternalLocation(s);
                            String lib = el != null ? el.getLibraryName() : "?";
                            apis.add(lib.toUpperCase().replaceAll("\\.DLL$", "") + "!" + s.getName());
                        }
                        continue;
                    }
                    RefType t = r.getReferenceType();
                    if (t.isCall() || (t.isJump() && !body.contains(to) && fm.getFunctionAt(to) != null)) {
                        Function c = fm.getFunctionAt(to);
                        if (c == null) continue;
                        if (c.isThunk()) {
                            Function tf = c.getThunkedFunction(true);
                            if (tf != null && tf.isExternal()) {
                                ExternalLocation el = tf.getExternalLocation();
                                String lib = el != null ? el.getLibraryName() : "?";
                                apis.add(lib.toUpperCase().replaceAll("\\.DLL$", "") + "!" + tf.getName());
                                continue;
                            }
                        }
                        if (!c.getEntryPoint().equals(f.getEntryPoint())) callees.add(c.getEntryPoint().getOffset());
                    } else if (t.isData()) {
                        if (body.contains(to)) continue;
                        Data d = listing.getDefinedDataAt(to);
                        if (d == null) d = listing.getDefinedDataContaining(to);
                        if (d != null && d.hasStringValue()) {
                            Object v = d.getValue();
                            strings.put(to.getOffset(), v == null ? "" : v.toString());
                        } else if (to.getOffset() >= 0x400000) {
                            MemoryBlock mb = currentProgram.getMemory().getBlock(to);
                            if (mb == null || mb.isExecute()) continue;
                            String raw = rawString(to);
                            if (raw != null && (d == null || d.getMinAddress().equals(to))) strings.put(to.getOffset(), raw);
                            else data.add(to.getOffset());
                        }
                    }
                }
            }
            long off = f.getEntryPoint().getOffset();
            if (!first) sb.append(",\n");
            first = false;
            sb.append("{\"addr\":").append(esc(hx(f.getEntryPoint())))
              .append(",\"size\":").append(body.getNumAddresses())
              .append(",\"name\":").append(esc(f.getName()))
              .append(",\"thunk\":").append(f.isThunk())
              .append(",\"fid\":").append(fidAddrs.contains(off))
              .append(",\"fid_name\":").append(esc(fidNames.getOrDefault(off, "")))
              .append(",\"callees\":").append(arr(callees))
              .append(",\"callers\":").append(arr(callers))
              .append(",\"apis\":[");
            boolean f1 = true;
            for (String s : apis) { if (!f1) sb.append(','); f1 = false; sb.append(esc(s)); }
            sb.append("],\"strings\":[");
            f1 = true;
            for (Map.Entry<Long, String> e : strings.entrySet()) {
                if (!f1) sb.append(','); f1 = false;
                sb.append("{\"addr\":").append(esc("0x" + Long.toHexString(e.getKey()))).append(",\"text\":").append(esc(e.getValue())).append('}');
            }
            sb.append("],\"data\":").append(arr(data)).append('}');
            n++;
        }
        sb.append("\n]\n}\n");
        Files.write(Paths.get(out), sb.toString().getBytes("UTF-8"));
        println("dumped " + n + " functions to " + out);
    }

    // Undefined bytes that look like a C string (>= 4 printable chars).
    String rawString(Address a) {
        try {
            StringBuilder b = new StringBuilder();
            for (int i = 0; i < 200; i++) {
                int c = currentProgram.getMemory().getByte(a.add(i)) & 0xff;
                if (c == 0) break;
                if (c < 0x20 || c > 0x7e) { if (c != 9 && c != 10 && c != 13) return null; }
                b.append((char) c);
            }
            return b.length() >= 4 ? b.toString() : null;
        } catch (Exception e) { return null; }
    }

    static String arr(Collection<Long> c) {
        StringBuilder b = new StringBuilder("[");
        boolean f = true;
        for (Long l : c) { if (!f) b.append(','); f = false; b.append('"').append("0x").append(Long.toHexString(l)).append('"'); }
        return b.append(']').toString();
    }
}
