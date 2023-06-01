import ExampleBDPI::*;

import Assert::*;

module mkTopModule(Empty);
    Reg#(Bit#(20)) stepNumber <- mkReg(0);

    ExampleBDPI_Ifc example <- mkExampleBDPI;

    (* no_implicit_conditions *)
    rule test;
        case(stepNumber)
            0: begin
                let value = example.call('h10);
                dynamicAssert(value == 'h110, "ExampleBDPI - call didn't return expected value");
            end

            default: begin
                dynamicAssert(stepNumber == 1, "ExampleBDPI - not all tests run");
                $display(">>>PASS");
                $finish();
            end
        endcase
    endrule

    rule increment_step_number;
        stepNumber <= stepNumber + 1;
    endrule
endmodule
