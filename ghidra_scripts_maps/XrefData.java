//@category Maps
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import ghidra.program.model.scalar.Scalar;
import java.util.*;

public class XrefData extends GhidraScript {
    @Override public void run() throws Exception {
        for (String a : getScriptArgs()) {
            Address t = toAddr(a);
            println("=== refs to " + a);
            ReferenceIterator ri = currentProgram.getReferenceManager().getReferencesTo(t);
            int n=0;
            while (ri.hasNext() && n<60) {
                Reference r = ri.next();
                Function f = getFunctionContaining(r.getFromAddress());
                Instruction ins = getInstructionAt(r.getFromAddress());
                println("   " + r.getFromAddress() + "  " + (f==null?"?":f.getName())
                        + "  " + r.getReferenceType() + "  | " + (ins==null?"":ins.toString()));
                n++;
            }
            if(n==0) println("   (none)");
        }
    }
}
