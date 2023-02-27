//!topmodule mkSimpleTest
import Simple::*;

module mkSimpleTest(Empty);
    Simple simple <- mkSimple;

    rule just_stop;
        $display(">>>PASS");
        $finish();
    endrule
endmodule
