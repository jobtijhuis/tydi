library ieee;
  use ieee.std_logic_1164.all;
  use ieee.numeric_std.all;

library vunit_lib;
  context vunit_lib.vunit_context;

entity tb_slice_complex_a is
  generic (
    RUNNER_CFG   : string := runner_cfg_default
    TEST_TIMEOUT : time   := 1 ms
  );
end entity tb_slice_complex_a;

architecture tb of tb_slice_complex_a is

  constant clk_period : time := 10 ns;

  signal clk       : std_logic                      := '0';
  signal rst       : std_logic                      := '1';
  signal in_valid  : std_logic                      := '0';
  signal in_ready  : std_logic;
  signal in_data   : std_logic_vector(263 downto 0) := (others => '0');
  signal in_stai   : std_logic_vector(2 downto 0)   := (others => '0');
  signal in_endi   : std_logic_vector(2 downto 0)   := (others => '0');
  signal in_strb   : std_logic_vector(7 downto 0)   := (others => '0');
  signal out_valid : std_logic;
  signal out_ready : std_logic                      := '0';
  signal out_data  : std_logic_vector(263 downto 0);
  signal out_stai  : std_logic_vector(2 downto 0);
  signal out_endi  : std_logic_vector(2 downto 0);
  signal out_strb  : std_logic_vector(7 downto 0);

begin

  clk <= not clk after clk_period / 2;

  dut : entity work.slice_complex_a
    port map (
      clk       => clk,
      rst       => rst,
      in_valid  => in_valid,
      in_ready  => in_ready,
      in_data   => in_data,
      in_stai   => in_stai,
      in_endi   => in_endi,
      in_strb   => in_strb,
      out_valid => out_valid,
      out_ready => out_ready,
      out_data  => out_data,
      out_stai  => out_stai,
      out_endi  => out_endi,
      out_strb  => out_strb
    );

  main : process is

    -- Drive one transfer into the slice and check it appears on the output.

    procedure send_and_check (
      data : std_logic_vector(263 downto 0);
      stai : std_logic_vector(2 downto 0);
      endi : std_logic_vector(2 downto 0);
      strb : std_logic_vector(7 downto 0)
    ) is
    begin

      -- Present the input word.
      in_data   <= data;
      in_stai   <= stai;
      in_endi   <= endi;
      in_strb   <= strb;
      in_valid  <= '1';
      out_ready <= '1';

      -- Wait until the slice accepts the input handshake.
      loop

        wait until rising_edge(clk);
        exit when in_ready = '1';

      end loop;

      in_valid <= '0';

      -- Wait until the slice presents valid output.
      loop

        exit when out_valid = '1';
        wait until rising_edge(clk);

      end loop;

      check_equal(out_data, data, "out_data mismatch");
      check_equal(out_stai, stai, "out_stai mismatch");
      check_equal(out_endi, endi, "out_endi mismatch");
      check_equal(out_strb, strb, "out_strb mismatch");

      wait until rising_edge(clk);

    end procedure send_and_check;

  begin

    test_runner_setup(runner, RUNNER_CFG);

    -- Reset.
    rst <= '1';
    wait for 4 * clk_period;
    wait until rising_edge(clk);
    rst <= '0';
    wait until rising_edge(clk);

    if run("test_single_transfer") then
      send_and_check(
                     data => std_logic_vector(to_unsigned(16#A5#, 264)),
                     stai => "001",
                     endi => "110",
                     strb => x"FF"
                   );
    elsif run("test_multiple_transfers") then

      for i in 0 to 7 loop

        send_and_check(
                       data => std_logic_vector(to_unsigned(i + 1, 264)),
                       stai => std_logic_vector(to_unsigned(i mod 8, 3)),
                       endi => std_logic_vector(to_unsigned((7 - i) mod 8, 3)),
                       strb => std_logic_vector(to_unsigned(i, 8))
                     );

      end loop;

    end if;

    test_runner_cleanup(runner);

  end process main;

  test_runner_watchdog(runner, 1 ms);

end architecture tb;
