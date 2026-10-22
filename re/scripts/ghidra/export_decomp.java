// Export decompiled C of every non-thunk function (see docs/re-workflow.md).
// Args: <outdir> <symbols.toml> <coverage.toml>
// @category RRT2
import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.io.*;
import java.nio.file.*;
import java.util.*;
import java.util.regex.*;

public class export_decomp extends GhidraScript {
    static final int TIMEOUT_SECS = 60;

    static String unesc(String s) { return s.replace("\\\\", "\\").replace("\\\"", "\""); }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        Path outDir = Paths.get(args[0]);
        Files.createDirectories(outDir);
        // Clean stale output so renamed functions do not leave duplicates.
        try (DirectoryStream<Path> ds = Files.newDirectoryStream(outDir, "*.c")) {
            for (Path p : ds) Files.delete(p);
        }

        // ---- symbols.toml: addr, name, kind, comment
        List<String[]> syms = new ArrayList<>(); // addr, name, kind, comment
        Pattern kv = Pattern.compile("^(\\w+)\\s*=\\s*\"(.*)\"\\s*$");
        List<String> lines = new ArrayList<>(Files.readAllLines(Paths.get(args[1])));
        lines.add("[[symbol]]");
        Map<String, String> cur = null;
        for (String l : lines) {
            if (l.startsWith("[[symbol]]")) {
                if (cur != null && cur.containsKey("addr") && cur.containsKey("name"))
                    syms.add(new String[] { cur.get("addr"), cur.get("name"), cur.getOrDefault("kind", "function"), cur.getOrDefault("comment", "") });
                cur = new HashMap<>();
                continue;
            }
            Matcher m = kv.matcher(l);
            if (cur != null && m.find()) cur.put(m.group(1), unesc(m.group(2)));
        }
        // ---- coverage.toml: addr -> category,status
        Map<Long, String[]> cov = new HashMap<>();
        Long ca = null; String cc = "unknown", cs = "unknown";
        lines = new ArrayList<>(Files.readAllLines(Paths.get(args[2])));
        lines.add("[[function]]");
        for (String l : lines) {
            if (l.startsWith("[[function]]")) {
                if (ca != null) cov.put(ca, new String[] { cc, cs });
                ca = null; cc = "unknown"; cs = "unknown";
                continue;
            }
            Matcher m = kv.matcher(l);
            if (m.find()) {
                if (m.group(1).equals("addr")) ca = Long.parseUnsignedLong(m.group(2).substring(2), 16);
                else if (m.group(1).equals("category")) cc = m.group(2);
                else if (m.group(1).equals("status")) cs = m.group(2);
            }
        }

        // ---- apply names and comments
        FunctionManager fm = currentProgram.getFunctionManager();
        Listing listing = currentProgram.getListing();
        SymbolTable st = currentProgram.getSymbolTable();
        int nf = 0, ng = 0;
        for (String[] s : syms) {
            Address a = toAddr(Long.parseUnsignedLong(s[0].substring(2), 16));
            try {
                Function f = fm.getFunctionAt(a);
                if (s[2].equals("function") && f != null) {
                    f.setName(s[1], SourceType.USER_DEFINED); nf++;
                    if (!s[3].isEmpty()) listing.setComment(a, CodeUnit.PLATE_COMMENT, s[3]);
                } else {
                    st.createLabel(a, s[1], SourceType.USER_DEFINED).setPrimary(); ng++;
                    if (!s[3].isEmpty()) listing.setComment(a, CodeUnit.EOL_COMMENT, s[3]);
                }
            } catch (Exception ex) { printerr("symbol " + s[1] + ": " + ex); }
        }
        println("applied " + nf + " function names, " + ng + " data labels");

        DecompInterface di = new DecompInterface();
        di.openProgram(currentProgram);
        int ok = 0, fail = 0, skipped = 0;
        StringBuilder index = new StringBuilder("addr\tname\tcategory\tstatus\tsize\tfile\n");
        StringBuilder failures = new StringBuilder();
        List<Function> funcs = new ArrayList<>();
        for (Function f : fm.getFunctions(true)) funcs.add(f);
        for (Function f : funcs) {
            if (f.isExternal() || f.isThunk()) { skipped++; continue; }
            long off = f.getEntryPoint().getOffset();
            String[] c = cov.getOrDefault(off, new String[] { "unknown", "unknown" });
            String name = f.getName();
            String fname = String.format("%08x_%s.c", off, name.replaceAll("[^A-Za-z0-9_.]", "_"));
            long size = f.getBody().getNumAddresses();
            String code = null, err = null;
            try {
                DecompileResults r = di.decompileFunction(f, TIMEOUT_SECS, monitor);
                if (r.decompileCompleted() && r.getDecompiledFunction() != null) code = r.getDecompiledFunction().getC();
                else err = r.getErrorMessage() == null ? "unknown error" : r.getErrorMessage().trim();
            } catch (Exception ex) { err = ex.toString(); }
            StringBuilder h = new StringBuilder("/*\n");
            h.append(" * addr:     0x").append(Long.toHexString(off)).append('\n');
            h.append(" * name:     ").append(name).append('\n');
            h.append(" * size:     ").append(size).append(" bytes\n");
            h.append(" * category: ").append(c[0]).append("  status: ").append(c[1]).append('\n');
            h.append(" * callers:  ").append(names(f.getCallingFunctions(monitor))).append('\n');
            h.append(" * callees:  ").append(names(f.getCalledFunctions(monitor))).append('\n');
            if (err != null) h.append(" * DECOMPILE FAILED: ").append(err.replace("*/", "* /")).append('\n');
            h.append(" */\n");
            Files.write(outDir.resolve(fname), (h + (code == null ? "" : code)).getBytes("UTF-8"));
            index.append(String.format("0x%x\t%s\t%s\t%s\t%d\t%s\n", off, name, c[0], c[1], size, fname));
            if (err == null) ok++; else { fail++; failures.append(String.format("0x%x\t%s\t%s\n", off, name, err.replace('\n', ' '))); }
        }
        Files.write(outDir.resolve("INDEX.tsv"), index.toString().getBytes("UTF-8"));
        Files.write(outDir.resolve("FAILURES.tsv"), failures.toString().getBytes("UTF-8"));
        di.dispose();
        println("exported " + ok + " functions, failed " + fail + ", skipped " + skipped + " thunks");
    }

    static String names(Set<Function> s) {
        TreeMap<Long, String> m = new TreeMap<>();
        for (Function f : s) if (!f.isExternal()) m.put(f.getEntryPoint().getOffset(), f.getName());
        if (m.isEmpty()) return "-";
        StringBuilder b = new StringBuilder();
        for (String n : m.values()) { if (b.length() > 0) b.append(", "); b.append(n); }
        return b.toString();
    }
}
