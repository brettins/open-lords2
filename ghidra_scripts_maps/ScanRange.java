//@category Maps
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.scalar.Scalar;
import java.util.*;

public class ScanRange extends GhidraScript {
    @Override public void run() throws Exception {
        String[] a = getScriptArgs();
        long lo = Long.decode(a[0]), hi = Long.decode(a[1]);
        Map<Long,List<String>> byVal = new TreeMap<>();
        InstructionIterator it = currentProgram.getListing().getInstructions(true);
        while (it.hasNext()) {
            Instruction ins = it.next();
            for (int i=0;i<ins.getNumOperands();i++)
                for (Object o : ins.getOpObjects(i))
                    if (o instanceof Scalar) {
                        long v=((Scalar)o).getUnsignedValue();
                        if (v>=lo && v<hi) {
                            Function f=getFunctionContaining(ins.getAddress());
                            byVal.computeIfAbsent(v,k->new ArrayList<>())
                                 .add(ins.getAddress()+" "+(f==null?"?":f.getName())+" | "+ins);
                        }
                    }
        }
        for (Map.Entry<Long,List<String>> e: byVal.entrySet()) {
            println("--- 0x"+Long.toHexString(e.getKey())+"  ("+e.getValue().size()+" refs)");
            int n=Math.min(e.getValue().size(),25);
            for(int i=0;i<n;i++) println("     "+e.getValue().get(i));
            if(e.getValue().size()>n) println("     ... "+(e.getValue().size()-n)+" more");
        }
    }
}
