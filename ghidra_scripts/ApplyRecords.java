// Apply the struct layouts in docs/records.json to the Ghidra database.
//
// Why this exists
// ---------------
// The game keeps five arrays of fixed-stride records - counties, units, realms,
// AI personalities, and the labour slots inside a county. Ghidra sees only the
// address arithmetic, so it invents one synthetic global per field:
// DAT_0053f9bc is not an unknown, it is county[i].happiness, and 334 of the
// binary's "unknown" globals are that. Typing the arrays collapses all of them
// at once, and every function that touches a record becomes readable without
// anyone naming that function.
//
// Like ApplySymbols this script contains no knowledge of the binary. The
// layouts live in docs/records.json, checked against the widths
// RecordProbe.java measures; adding a field is an edit to the JSON.
//
//   analyzeHeadless E:\dev\ghidra-projects lords2 -process Lords2.exe -noanalysis \
//       -scriptPath E:\dev\lords2\ghidra_scripts -postScript ApplyRecords.java [path] [--dry-run]
//
// With no path argument it looks for ../docs/records.json beside its own source.
//@category Lords2

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.ArrayDataType;
import ghidra.program.model.data.ByteDataType;
import ghidra.program.model.data.CategoryPath;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.DataTypeConflictHandler;
import ghidra.program.model.data.DataTypeManager;
import ghidra.program.model.data.IntegerDataType;
import ghidra.program.model.data.ShortDataType;
import ghidra.program.model.data.SignedByteDataType;
import ghidra.program.model.data.Structure;
import ghidra.program.model.data.StructureDataType;
import ghidra.program.model.data.UnsignedIntegerDataType;
import ghidra.program.model.data.UnsignedShortDataType;
import ghidra.program.model.symbol.SourceType;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.HashMap;
import java.util.Map;

public class ApplyRecords extends GhidraScript {

    private static final CategoryPath CAT = new CategoryPath("/lords2");

    private boolean dryRun = false;
    private final Map<String, DataType> defined = new HashMap<>();
    private int structs = 0, fields = 0, arrays = 0, failed = 0;

    @Override
    public void run() throws Exception {
        File json = null;
        for (String a : getScriptArgs()) {
            if (a.equals("--dry-run")) dryRun = true;
            else json = new File(a);
        }
        if (json == null) json = defaultJson();
        if (json == null || !json.isFile()) {
            printerr("records.json not found" + (json == null ? "" : ": " + json));
            return;
        }
        println("### reading " + json.getAbsolutePath() + (dryRun ? "  (dry run)" : ""));

        JsonObject root = JsonParser.parseString(
                new String(Files.readAllBytes(json.toPath()), StandardCharsets.UTF_8)).getAsJsonObject();

        for (JsonElement e : root.getAsJsonArray("structs")) defineStruct(e.getAsJsonObject());
        for (JsonElement e : root.getAsJsonArray("arrays")) applyArray(e.getAsJsonObject());

        println("### structs " + structs + ", fields " + fields
                + ", arrays applied " + arrays + ", failed " + failed);
        if (failed > 0) printerr("### " + failed + " failure(s)");
    }

    /** ghidra_scripts/ApplyRecords.java -> ../docs/records.json */
    private File defaultJson() {
        try {
            File src = getSourceFile().getFile(false);
            File repo = src.getParentFile().getParentFile();
            return new File(new File(repo, "docs"), "records.json");
        } catch (Exception e) {
            return null;
        }
    }

    private String str(JsonObject o, String key) {
        return o.has(key) && !o.get(key).isJsonNull() ? o.get(key).getAsString() : null;
    }

    // ------------------------------------------------------------------ types

    /**
     * Resolve a type spelling from the JSON. Scalars are the six widths the
     * binary actually uses; a trailing [n] makes an array; anything else must
     * be a struct already defined earlier in the file.
     */
    private DataType type(String spec) {
        String s = spec.trim();
        int lb = s.indexOf('[');
        if (lb >= 0) {
            int n = Integer.parseInt(s.substring(lb + 1, s.indexOf(']')).trim());
            DataType elem = type(s.substring(0, lb));
            return elem == null ? null : new ArrayDataType(elem, n, elem.getLength());
        }
        switch (s) {
            case "u8":  return ByteDataType.dataType;
            case "i8":  return SignedByteDataType.dataType;
            case "u16": return UnsignedShortDataType.dataType;
            case "i16": return ShortDataType.dataType;
            case "u32": return UnsignedIntegerDataType.dataType;
            case "i32": return IntegerDataType.dataType;
            default:    return defined.get(s);
        }
    }

    private void defineStruct(JsonObject s) {
        String name = str(s, "name");
        int size = s.get("size").getAsInt();

        // A fixed-size structure starts as `size` undefined bytes, and every
        // offset this file does not name stays that way. The gaps are the
        // point: they are what is still unknown, and they are visible.
        StructureDataType st = new StructureDataType(CAT, name, size);
        String comment = str(s, "comment");
        if (comment != null) st.setDescription(comment);

        JsonArray fs = s.getAsJsonArray("fields");
        int placed = 0;
        for (JsonElement e : fs) {
            JsonObject f = e.getAsJsonObject();
            int off = Integer.decode(str(f, "off"));
            String fname = str(f, "name");
            String spec = str(f, "type");
            DataType dt = type(spec);
            if (dt == null) {
                printerr("  " + name + "." + fname + ": unknown type " + spec);
                failed++;
                continue;
            }
            if (off + dt.getLength() > size) {
                printerr("  " + name + "." + fname + " at 0x" + Integer.toHexString(off)
                        + " runs past the record's " + size + " bytes");
                failed++;
                continue;
            }
            try {
                st.replaceAtOffset(off, dt, dt.getLength(), fname, str(f, "comment"));
                placed++;
            } catch (Exception ex) {
                printerr("  " + name + "." + fname + " at 0x" + Integer.toHexString(off)
                        + ": " + ex.getMessage());
                failed++;
            }
        }

        DataTypeManager dtm = currentProgram.getDataTypeManager();
        DataType stored = dryRun ? st
                : dtm.addDataType(st, DataTypeConflictHandler.REPLACE_HANDLER);
        defined.put(name, stored);
        structs++;
        fields += placed;

        // How much of the record we can actually account for. A record that is
        // mostly padding is a record we do not understand yet, and saying so is
        // the honest version of "typed".
        int known = 0;
        for (ghidra.program.model.data.DataTypeComponent c : st.getDefinedComponents()) {
            if (c.getFieldName() != null) known += c.getLength();
        }
        println(String.format("  struct %-14s %3d fields, %4d/%4d bytes named (%d%%)",
                name, placed, known, size, known * 100 / size));
    }

    // ----------------------------------------------------------------- arrays

    private void applyArray(JsonObject a) {
        String label = str(a, "name");
        Address addr = toAddr(Long.decode(str(a, "addr")));
        DataType elem = defined.get(str(a, "struct"));
        int count = a.get("count").getAsInt();
        if (elem == null) {
            printerr("  " + label + ": struct " + str(a, "struct") + " was not defined");
            failed++;
            return;
        }
        ArrayDataType arr = new ArrayDataType(elem, count, elem.getLength());
        Address end = addr.add((long) elem.getLength() * count - 1);
        if (!currentProgram.getMemory().contains(end)) {
            printerr("  " + label + ": " + addr + ".." + end + " is not all in memory");
            failed++;
            return;
        }
        if (dryRun) {
            println(String.format("  array  %-16s %s..%s  %d x %d", label, addr, end,
                    count, elem.getLength()));
            arrays++;
            return;
        }
        try {
            // Ghidra will not lay data over data. Everything in the range is
            // either undefined or an analyser guess at a scalar, so clearing is
            // safe; labels survive it and stay as interior names.
            clearListing(addr, end);
            createData(addr, arr);
            createLabel(addr, label, true, SourceType.USER_DEFINED);
            setPlateComment(addr, count + " x " + elem.getLength() + "-byte "
                    + str(a, "struct") + " records"
                    + "\n(layout from docs/records.json - lords2 project, not an original symbol)");
            println(String.format("  array  %-16s %s..%s  %d x %d", label, addr, end,
                    count, elem.getLength()));
            arrays++;
        } catch (Exception ex) {
            printerr("  " + label + ": " + ex);
            failed++;
        }
    }
}
