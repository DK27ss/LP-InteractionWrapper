// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

interface IERC20 {
    function approve(address spender, uint256 amount) external returns (bool);
    function transfer(address to, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
    function decimals() external view returns (uint8);
    function symbol() external view returns (string memory);
}

interface IPancakeV3Pool {
    function slot0() external view returns (uint160, int24, uint16, uint16, uint16, uint32, bool);
    function token0() external view returns (address);
    function token1() external view returns (address);
    function fee() external view returns (uint24);
    function swap(address recipient, bool zeroForOne, int256 amountSpecified, uint160 sqrtPriceLimitX96, bytes calldata data) external returns (int256, int256);
}

interface INonfungiblePositionManager {
    struct MintParams {
        address token0;
        address token1;
        uint24 fee;
        int24 tickLower;
        int24 tickUpper;
        uint256 amount0Desired;
        uint256 amount1Desired;
        uint256 amount0Min;
        uint256 amount1Min;
        address recipient;
        uint256 deadline;
    }

    struct DecreaseLiquidityParams {
        uint256 tokenId;
        uint128 liquidity;
        uint256 amount0Min;
        uint256 amount1Min;
        uint256 deadline;
    }

    struct CollectParams {
        uint256 tokenId;
        address recipient;
        uint128 amount0Max;
        uint128 amount1Max;
    }

    function mint(MintParams calldata params) external payable returns (uint256 tokenId, uint128 liquidity, uint256 amount0, uint256 amount1);
    function decreaseLiquidity(DecreaseLiquidityParams calldata params) external payable returns (uint256 amount0, uint256 amount1);
    function collect(CollectParams calldata params) external payable returns (uint256 amount0, uint256 amount1);
    function burn(uint256 tokenId) external payable;
    function positions(uint256 tokenId) external view returns (
        uint96 nonce, address operator, address token0, address token1, uint24 fee,
        int24 tickLower, int24 tickUpper, uint128 liquidity,
        uint256 feeGrowthInside0LastX128, uint256 feeGrowthInside1LastX128,
        uint128 tokensOwed0, uint128 tokensOwed1
    );
}

interface IERC721Receiver {
    function onERC721Received(address, address, uint256, bytes calldata) external returns (bytes4);
}

contract UniversalTrigger is IERC721Receiver {
    address public constant POSITION_MANAGER = 0x7b8A01B39D58278b5DE7e48c8449c9f4F5170613;
    address public owner;
    address public targetPool;
    address public baseToken;
    address public quoteToken;
    uint24 public poolFee;
    int24 public defaultTickLower = -9200;
    int24 public defaultTickUpper = -7000;

    uint256[] public positions;
    mapping(uint256 => bool) public isOurPosition;

    event PoolConfigured(address indexed pool, address baseToken, address quoteToken, uint24 fee);
    event PositionCreated(uint256 indexed tokenId, int24 tickLower, int24 tickUpper, uint128 liquidity);
    event PositionClosed(uint256 indexed tokenId);
    event Rebalanced(uint256 indexed oldTokenId, uint256 indexed newTokenId, int24 newTickLower, int24 newTickUpper);

    constructor() {
        owner = msg.sender;
    }

    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }

    function setPool(address _pool) external onlyOwner {
        require(_pool != address(0), "Invalid pool");
        targetPool = _pool;

        baseToken = IPancakeV3Pool(_pool).token0();
        quoteToken = IPancakeV3Pool(_pool).token1();
        poolFee = IPancakeV3Pool(_pool).fee();

        emit PoolConfigured(_pool, baseToken, quoteToken, poolFee);
    }

    function setPoolFull(address _pool, address _baseToken, address _quoteToken) external onlyOwner {
        require(_pool != address(0), "Invalid pool");
        targetPool = _pool;
        baseToken = _baseToken;
        quoteToken = _quoteToken;
        poolFee = IPancakeV3Pool(_pool).fee();

        emit PoolConfigured(_pool, _baseToken, _quoteToken, poolFee);
    }

    function setDefaultTicks(int24 _lower, int24 _upper) external onlyOwner {
        require(_lower < _upper, "Invalid ticks");
        defaultTickLower = _lower;
        defaultTickUpper = _upper;
    }

    function onERC721Received(address, address, uint256, bytes calldata) external pure override returns (bytes4) {
        return IERC721Receiver.onERC721Received.selector;
    }

    function addPosition(
        uint256 baseAmount,
        uint256 quoteAmount,
        int24 tickLower,
        int24 tickUpper
    ) external onlyOwner returns (uint256 tokenId) {
        require(targetPool != address(0), "Pool not configured");
        require(IERC20(baseToken).balanceOf(address(this)) >= baseAmount, "Not enough base");
        require(IERC20(quoteToken).balanceOf(address(this)) >= quoteAmount, "Not enough quote");

        (tokenId,) = _mintPosition(tickLower, tickUpper, baseAmount, quoteAmount);
        positions.push(tokenId);
        isOurPosition[tokenId] = true;
    }

    function addPositionDefault(uint256 baseAmount, uint256 quoteAmount) external onlyOwner returns (uint256 tokenId) {
        require(targetPool != address(0), "Pool not configured");
        require(IERC20(baseToken).balanceOf(address(this)) >= baseAmount, "Not enough base");
        require(IERC20(quoteToken).balanceOf(address(this)) >= quoteAmount, "Not enough quote");

        (tokenId,) = _mintPosition(defaultTickLower, defaultTickUpper, baseAmount, quoteAmount);
        positions.push(tokenId);
        isOurPosition[tokenId] = true;
    }

    function addPositionBatch(uint256 basePerPos, uint256 quotePerPos, uint256 count) external onlyOwner returns (uint256[] memory tokenIds) {
        require(targetPool != address(0), "Pool not configured");
        require(count > 0 && count <= 20, "Count 1-20");
        require(IERC20(baseToken).balanceOf(address(this)) >= basePerPos * count, "Not enough base");
        require(IERC20(quoteToken).balanceOf(address(this)) >= quotePerPos * count, "Not enough quote");

        tokenIds = new uint256[](count);
        for (uint256 i = 0; i < count; i++) {
            (uint256 tokenId,) = _mintPosition(defaultTickLower, defaultTickUpper, basePerPos, quotePerPos);
            positions.push(tokenId);
            isOurPosition[tokenId] = true;
            tokenIds[i] = tokenId;
        }
    }

    function closePosition(uint256 tokenId) external onlyOwner {
        require(isOurPosition[tokenId], "Not our position");
        _closePosition(tokenId);
    }

    function closeAllPositions() external onlyOwner {
        _closePositions(positions.length);
    }

    function closeBatchPositions(uint256 count) external onlyOwner {
        _closePositions(count);
    }

    function rebalance(uint256 tokenId, int24 newTickLower, int24 newTickUpper) external onlyOwner returns (uint256 newTokenId) {
        require(isOurPosition[tokenId], "Not our position");

        _closePosition(tokenId);

        uint256 baseBal = IERC20(baseToken).balanceOf(address(this));
        uint256 quoteBal = IERC20(quoteToken).balanceOf(address(this));
        require(baseBal > 0 && quoteBal > 0, "No tokens");

        (newTokenId,) = _mintPosition(newTickLower, newTickUpper, baseBal, quoteBal);
        positions.push(newTokenId);
        isOurPosition[newTokenId] = true;

        emit Rebalanced(tokenId, newTokenId, newTickLower, newTickUpper);
    }

    function rebalanceDefault(uint256 tokenId) external onlyOwner returns (uint256 newTokenId) {
        require(isOurPosition[tokenId], "Not our position");

        _closePosition(tokenId);
        uint256 baseBal = IERC20(baseToken).balanceOf(address(this));
        uint256 quoteBal = IERC20(quoteToken).balanceOf(address(this));
        require(baseBal > 0 && quoteBal > 0, "No tokens");

        (newTokenId,) = _mintPosition(defaultTickLower, defaultTickUpper, baseBal, quoteBal);
        positions.push(newTokenId);
        isOurPosition[newTokenId] = true;

        emit Rebalanced(tokenId, newTokenId, defaultTickLower, defaultTickUpper);
    }

    function rebalanceBatch(uint256 count, int24 newTickLower, int24 newTickUpper) external onlyOwner returns (uint256[] memory newTokenIds) {
        require(count > 0 && count <= 10, "Count 1-10");
        uint256[] memory toRebalance = new uint256[](count);
        uint256 found = 0;

        for (uint i = 0; i < positions.length && found < count; i++) {
            uint256 tokenId = positions[i];
            if (isOurPosition[tokenId]) {
                (,,,,,,,uint128 liquidity,,,,) = INonfungiblePositionManager(POSITION_MANAGER).positions(tokenId);
                if (liquidity > 0) {
                    toRebalance[found] = tokenId;
                    found++;
                }
            }
        }
        require(found == count, "Not enough positions");

        for (uint i = 0; i < count; i++) {
            _closePosition(toRebalance[i]);
        }

        uint256 baseBal = IERC20(baseToken).balanceOf(address(this));
        uint256 quoteBal = IERC20(quoteToken).balanceOf(address(this));
        uint256 basePerPos = baseBal / count;
        uint256 quotePerPos = quoteBal / count;
        require(basePerPos > 0 && quotePerPos > 0, "Not enough tokens");

        newTokenIds = new uint256[](count);
        for (uint i = 0; i < count; i++) {
            (uint256 newTokenId,) = _mintPosition(newTickLower, newTickUpper, basePerPos, quotePerPos);
            positions.push(newTokenId);
            isOurPosition[newTokenId] = true;
            newTokenIds[i] = newTokenId;
            emit Rebalanced(toRebalance[i], newTokenId, newTickLower, newTickUpper);
        }
    }

    function rebalanceBatchDefault(uint256 count) external onlyOwner returns (uint256[] memory) {
        return this.rebalanceBatch(count, defaultTickLower, defaultTickUpper);
    }

    function swapBaseForQuote(uint256 amountIn) external onlyOwner {
        require(targetPool != address(0), "Pool not configured");
        _swap(baseToken, quoteToken, amountIn);
    }

    function swapQuoteForBase(uint256 amountIn) external onlyOwner {
        require(targetPool != address(0), "Pool not configured");
        _swap(quoteToken, baseToken, amountIn);
    }

    function swapBaseForQuoteBatch(uint256 amountPerSwap, uint256 count) external onlyOwner {
        require(count > 0 && count <= 20, "Count 1-20");
        for (uint256 i = 0; i < count; i++) {
            _swap(baseToken, quoteToken, amountPerSwap);
        }
    }

    function swapQuoteForBaseBatch(uint256 amountPerSwap, uint256 count) external onlyOwner {
        require(count > 0 && count <= 20, "Count 1-20");
        for (uint256 i = 0; i < count; i++) {
            _swap(quoteToken, baseToken, amountPerSwap);
        }
    }

    function _mintPosition(int24 tickLower, int24 tickUpper, uint256 baseAmt, uint256 quoteAmt)
        internal returns (uint256 tokenId, uint128 liquidity)
    {
        address token0 = IPancakeV3Pool(targetPool).token0();
        address token1 = IPancakeV3Pool(targetPool).token1();
        (uint256 amt0, uint256 amt1) = token0 == baseToken ? (baseAmt, quoteAmt) : (quoteAmt, baseAmt);
        IERC20(token0).approve(POSITION_MANAGER, amt0);
        IERC20(token1).approve(POSITION_MANAGER, amt1);

        (tokenId, liquidity,,) = INonfungiblePositionManager(POSITION_MANAGER).mint(
            INonfungiblePositionManager.MintParams({
                token0: token0,
                token1: token1,
                fee: poolFee,
                tickLower: tickLower,
                tickUpper: tickUpper,
                amount0Desired: amt0,
                amount1Desired: amt1,
                amount0Min: 0,
                amount1Min: 0,
                recipient: address(this),
                deadline: block.timestamp + 300
            })
        );

        emit PositionCreated(tokenId, tickLower, tickUpper, liquidity);
    }

    function _closePosition(uint256 tokenId) internal {
        (,,,,,,,uint128 liquidity,,,,) = INonfungiblePositionManager(POSITION_MANAGER).positions(tokenId);

        if (liquidity > 0) {
            INonfungiblePositionManager(POSITION_MANAGER).decreaseLiquidity(
                INonfungiblePositionManager.DecreaseLiquidityParams({
                    tokenId: tokenId,
                    liquidity: liquidity,
                    amount0Min: 0,
                    amount1Min: 0,
                    deadline: block.timestamp + 300
                })
            );
        }

        INonfungiblePositionManager(POSITION_MANAGER).collect(
            INonfungiblePositionManager.CollectParams({
                tokenId: tokenId,
                recipient: address(this),
                amount0Max: type(uint128).max,
                amount1Max: type(uint128).max
            })
        );

        INonfungiblePositionManager(POSITION_MANAGER).burn(tokenId);
        isOurPosition[tokenId] = false;
        emit PositionClosed(tokenId);
    }

    function _closePositions(uint256 maxCount) internal {
        uint256 closed = 0;
        for (uint i = 0; i < positions.length && closed < maxCount; i++) {
            uint256 tokenId = positions[i];
            if (isOurPosition[tokenId]) {
                _closePosition(tokenId);
                closed++;
            }
        }
    }

    function _swap(address tokenIn, address tokenOut, uint256 amountIn) internal {
        address token0 = IPancakeV3Pool(targetPool).token0();
        bool zeroForOne = tokenIn == token0;
        uint160 sqrtPriceLimitX96 = zeroForOne
            ? 4295128740
            : 1461446703485210103287273052203988822378723970341;

        IERC20(tokenIn).approve(targetPool, type(uint256).max);
        IPancakeV3Pool(targetPool).swap(
            address(this),
            zeroForOne,
            int256(amountIn),
            sqrtPriceLimitX96,
            ""
        );
    }

    function uniswapV3SwapCallback(int256 amount0Delta, int256 amount1Delta, bytes calldata) external {
        require(msg.sender == targetPool, "Not pool");
        if (amount0Delta > 0) {
            IERC20(IPancakeV3Pool(targetPool).token0()).transfer(msg.sender, uint256(amount0Delta));
        }
        if (amount1Delta > 0) {
            IERC20(IPancakeV3Pool(targetPool).token1()).transfer(msg.sender, uint256(amount1Delta));
        }
    }

    function pancakeV3SwapCallback(int256 amount0Delta, int256 amount1Delta, bytes calldata) external {
        require(msg.sender == targetPool, "Not pool");
        if (amount0Delta > 0) {
            IERC20(IPancakeV3Pool(targetPool).token0()).transfer(msg.sender, uint256(amount0Delta));
        }
        if (amount1Delta > 0) {
            IERC20(IPancakeV3Pool(targetPool).token1()).transfer(msg.sender, uint256(amount1Delta));
        }
    }

    function getCurrentTick() external view returns (int24 tick) {
        require(targetPool != address(0), "Pool not configured");
        (, tick,,,,,) = IPancakeV3Pool(targetPool).slot0();
    }

    function getBalances() external view returns (uint256 baseBalance, uint256 quoteBalance) {
        if (baseToken != address(0)) {
            baseBalance = IERC20(baseToken).balanceOf(address(this));
        }
        if (quoteToken != address(0)) {
            quoteBalance = IERC20(quoteToken).balanceOf(address(this));
        }
    }

    function getPoolInfo() external view returns (
        address pool,
        address base,
        address quote,
        uint24 fee,
        int24 currentTick
    ) {
        pool = targetPool;
        base = baseToken;
        quote = quoteToken;
        fee = poolFee;
        if (targetPool != address(0)) {
            (, currentTick,,,,,) = IPancakeV3Pool(targetPool).slot0();
        }
    }

    function getActivePositions() external view returns (uint256[] memory activeIds, uint128[] memory liquidities) {
        uint256 count = 0;
        for (uint i = 0; i < positions.length; i++) {
            if (isOurPosition[positions[i]]) count++;
        }

        activeIds = new uint256[](count);
        liquidities = new uint128[](count);
        uint256 idx = 0;
        for (uint i = 0; i < positions.length; i++) {
            uint256 tokenId = positions[i];
            if (isOurPosition[tokenId]) {
                (,,,,,,,uint128 liq,,,,) = INonfungiblePositionManager(POSITION_MANAGER).positions(tokenId);
                activeIds[idx] = tokenId;
                liquidities[idx] = liq;
                idx++;
            }
        }
    }

    function deposit(address token, uint256 amount) external onlyOwner {
        IERC20(token).transferFrom(msg.sender, address(this), amount);
    }

    function withdraw(address token, uint256 amount) external onlyOwner {
        IERC20(token).transfer(owner, amount);
    }

    function withdrawAll() external onlyOwner {
        if (baseToken != address(0)) {
            uint256 baseBal = IERC20(baseToken).balanceOf(address(this));
            if (baseBal > 0) IERC20(baseToken).transfer(owner, baseBal);
        }
        if (quoteToken != address(0)) {
            uint256 quoteBal = IERC20(quoteToken).balanceOf(address(this));
            if (quoteBal > 0) IERC20(quoteToken).transfer(owner, quoteBal);
        }
    }

    function withdrawToken(address token) external onlyOwner {
        uint256 bal = IERC20(token).balanceOf(address(this));
        if (bal > 0) IERC20(token).transfer(owner, bal);
    }

    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "Invalid");
        owner = newOwner;
    }

    receive() external payable {}
}
