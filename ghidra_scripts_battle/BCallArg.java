// Find calls to a target function and report the constant pushed immediately before the CALL.
// args: outFile hexTargetFunc [hexTargetFunc ...]
//@category Lords2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.scalar.Scalar;
import java.io.*;
import java.util.*;

public class BCallArg extends GhidraScript {
    @Override public void run() throws Exception {
        String[] a = getScriptArgs();
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        for (int i = 1; i < a.length; i++) {
            Function t = getFunctionContaining(toAddr(a[i]));
            out.println("### calls to " + (t == null ? a[i] : t.getEntryPoint() + " " + t.getName()));
            if (t == null) continue;
            for (Reference r : getReferencesTo(t.getEntryPoint())) {
                Instruction call = getInstructionAt(r.getFromAddress());
                if (call == null) continue;
                // walk back up to 12 instructions collecting PUSH immediates
                List<String> pushes = new ArrayList<>();
                Instruction ins = call.getPrevious();
                for (int k = 0; k < 12 && ins != null; k++, ins = ins.getPrevious()) {
                    if (!ins.getMnemonicString().toUpperCase().startsWith("PUSH")) break;
                    Object[] o = ins.getOpObjects(0);
                    if (o.length == 1 && o[0] instanceof Scalar) pushes.add("0x" + Long.toHexString(((Scalar)o[0]).getUnsignedValue()));
                    else pushes.add(ins.getDefaultOperandRepresentation(0));
                }
                Function f = getFunctionContaining(r.getFromAddress());
                out.println("  " + r.getFromAddress() + "  in " + (f==null?"(none)":f.getEntryPoint()+" "+f.getName()) + "  args(last-pushed-first)=" + pushes);
            }
        }
        out.close();
        println("### wrote " + a[0]);
    }
}
