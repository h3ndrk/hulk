-- local inspect = require 'inspect'
print("Hello world from lua!")

function spawn_robot(number)
    table.insert(state.robots, create_robot(number))
end

spawn_robot(7)

local game_end_time = -1.0

function on_goal()
    print("Goal scored, resetting ball!")
    -- print("Ball: " .. inspect(state.ball))
    print("Ball was at x: " .. state.ball.position[1] .. " y: " .. state.ball.position[2])
    state.ball = nil
    game_end_time = state.cycle_count + 200
end

state.ball = {
    position = { 0.0, 0.0 },
    velocity = { 0.0, 0.0 },
}

function on_cycle()
    if state.cycle_count % 1000 == 0 then
        -- print(inspect(state))
    end

    if state.cycle_count == 100 then
        state.game_controller_state.game_state = "Ready"
    end

    if state.cycle_count == 1600 then
        state.game_controller_state.game_state = "Set"
    end

    if state.cycle_count == 1700 then
        state.game_controller_state.game_state = "Playing"
    end

    if state.cycle_count == 1900 then
        set_robot_pose(7, { -3, 0 }, 0)
        state.ball = {
            position = { -2.0, 0.0 },
            velocity = { 9.0, 2.0 },
        }
    end

    if state.cycle_count == 2000 then
        state.ball = None
    end

    if state.cycle_count == 6000 then
        state.finished = true
    end
    if state.cycle_count == game_end_time then
        state.finished = true
    end
end
