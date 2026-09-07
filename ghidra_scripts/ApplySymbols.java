// Apply the names in docs/symbols.json to the Ghidra database.
//
// The script contains no knowledge of the binary: everything comes from the JSON.
// Adding a name later is a one-line edit in docs/symbols.json, not a code change.
//
//   analyzeHeadless E:\dev\ghidra-projects lords2 -process Lords2.exe -noanalysis \
//       -scriptPath E:\dev\lords2\ghidra_scripts -postScript ApplySymbols.java [path] [--dry-run]
//
// With no path argument the script looks for ../docs/symbols.json beside its own source.
//@category Lords2

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import ghidra.app.cmd.function.ApplyFunctionSignatureCmd;
import ghidra.app.script.GhidraScript;
import ghidra.app.util.parser.FunctionSignatureParser;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.FunctionDefinitionDataType;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolTable;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;

public class ApplySymbols extends GhidraScript {

    private boolean dryRun = false;
    private int named = 0, signed = 0, commented = 0, labelled = 0, skipped = 0, failed = 0;

    @Override
    public void run() throws Exception {
        File json = null;
        for (String a : getScriptArgs()) {
            if (a.equals("--dry-run")) dryRun = true;
            else json = new File(a);
        }
        if (json == null) json = defaultJson();
        if (json == null || !json.isFile()) {
            printerr("symbols.json not found" + (json == null ? "" : ": " + json));
            return;
        }
        println("### reading " + json.getAbsolutePath() + (dryRun ? "  (dry run)" : ""));

        JsonObject root = JsonParser.parseString(
                new String(Files.readAllBytes(json.toPath()), StandardCharsets.UTF_8)).getAsJsonObject();

        checkImageBase(root);

        for (JsonElement e : array(root, "functions")) applyFunction(e.getAsJsonObject());
        for (JsonElement e : array(root, "globals"))   applyGlobal(e.getAsJsonObject());

        println("### functions named " + named + ", signatures applied " + signed
                + ", plate comments " + commented + ", globals labelled " + labelled
                + ", already correct " + skipped + ", failed " + failed);
        if (failed > 0) printerr("### " + failed + " entr" + (failed == 1 ? "y" : "ies") + " failed");
    }

    /** ghidra_scripts/ApplySymbols.java -> ../docs/symbols.json */
    private File defaultJson() {
        try {
            File src = getSourceFile().getFile(false);          // .../ghidra_scripts/ApplySymbols.java
            File repo = src.getParentFile().getParentFile();    // .../lords2
            return new File(new File(repo, "docs"), "symbols.json");
        } catch (Exception e) {
            return null;
        }
    }

    private JsonArray array(JsonObject o, String key) {
        return o.has(key) ? o.getAsJsonArray(key) : new JsonArray();
    }

    private String str(JsonObject o, String key) {
        return o.has(key) && !o.get(key).isJsonNull() ? o.get(key).getAsString() : null;
    }

    private void checkImageBase(JsonObject root) {
        String want = str(root, "imageBase");
        if (want == null) return;
        long have = currentProgram.getImageBase().getOffset();
        long expect = Long.decode(want);
        if (have != expect) {
            printerr("### image base mismatch: program is 0x" + Long.toHexString(have)
                    + ", symbols.json says " + want + " - addresses will be wrong");
        }
    }

    private void applyFunction(JsonObject f) {
        String addrText = str(f, "addr"), name = str(f, "name");
        Address addr = toAddr(Long.decode(addrText));
        Function fn = getFunctionAt(addr);
        if (fn == null) fn = getFunctionContaining(addr);
        if (fn == null) {
            printerr("  no function at " + addrText + " (" + name + ")");
            failed++;
            return;
        }
        if (!fn.getEntryPoint().equals(addr)) {
            printerr("  " + addrText + " (" + name + ") is inside " + fn.getName()
                    + ", not its entry point");
            failed++;
            return;
        }
        try {
            if (name.equals(fn.getName())) {
                skipped++;
            } else {
                if (!dryRun) fn.setName(name, SourceType.USER_DEFINED);
                println("  fn  " + addrText + "  " + name);
                named++;
            }

            String sig = str(f, "signature");
            if (sig != null && !dryRun) {
                sig = sig.trim();
                while (sig.endsWith(";")) sig = sig.substring(0, sig.length() - 1).trim();
                FunctionSignatureParser parser =
                        new FunctionSignatureParser(currentProgram.getDataTypeManager(), null);
                FunctionDefinitionDataType def = parser.parse(fn.getSignature(), sig);
                if (def != null) {
                    ApplyFunctionSignatureCmd cmd = new ApplyFunctionSignatureCmd(
                            addr, def, SourceType.USER_DEFINED);
                    if (cmd.applyTo(currentProgram, monitor)) signed++;
                    else printerr("  signature rejected for " + name + ": " + cmd.getStatusMsg());
                }
            } else if (sig != null) {
                signed++;
            }

            String comment = plate(f);
            if (comment != null) {
                if (!dryRun) setPlateComment(addr, comment);
                commented++;
            }
        } catch (Exception e) {
            printerr("  failed " + addrText + " (" + name + "): " + e);
            failed++;
        }
    }

    private void applyGlobal(JsonObject g) {
        String addrText = str(g, "addr"), name = str(g, "name");
        Address addr = toAddr(Long.decode(addrText));
        if (!currentProgram.getMemory().contains(addr)) {
            printerr("  " + addrText + " (" + name + ") is not in memory");
            failed++;
            return;
        }
        try {
            SymbolTable st = currentProgram.getSymbolTable();
            Symbol existing = st.getPrimarySymbol(addr);
            if (existing != null && name.equals(existing.getName())) {
                skipped++;
            } else {
                if (!dryRun) createLabel(addr, name, true, SourceType.USER_DEFINED);
                println("  var " + addrText + "  " + name);
                labelled++;
            }
            String comment = plate(g);
            if (comment != null) {
                if (!dryRun) setPlateComment(addr, comment);
                commented++;
            }
        } catch (Exception e) {
            printerr("  failed " + addrText + " (" + name + "): " + e);
            failed++;
        }
    }

    /** Plate comment text: the confidence tag plus whatever the data file says. */
    private String plate(JsonObject o) {
        String c = str(o, "comment");
        String conf = str(o, "confidence");
        if (c == null && conf == null) return null;
        StringBuilder sb = new StringBuilder();
        if (conf != null) sb.append("[").append(conf).append("] ");
        if (c != null) sb.append(c);
        sb.append("\n(name from docs/symbols.json - lords2 project, not an original symbol)");
        return sb.toString();
    }
}
